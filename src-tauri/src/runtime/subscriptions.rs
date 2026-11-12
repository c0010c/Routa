use super::*;
use crate::model::{ExitMember, NodeRef};
use crate::subscription::review::{self, SubscriptionPreview};

pub(super) struct DeletedSnapshot {
    token: String,
    before: Model,
    after: Model,
    at: Instant,
}

#[derive(serde::Serialize, specta::Type)]
pub struct SubscriptionDeleted {
    pub token: String,
    pub outcome: ApplyOutcome,
}

impl Runtime {
    /// Downloads and validates without replacing nodes or changing network settings.
    pub async fn preview_subscription(
        &self,
        mut sub: Subscription,
    ) -> Result<SubscriptionPreview, String> {
        let url = url::Url::parse(&sub.url).map_err(|_| "订阅地址无效")?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("订阅地址必须使用 HTTP 或 HTTPS".into());
        }
        let snapshot = self.model();
        if snapshot
            .subscriptions
            .iter()
            .any(|s| s.id != sub.id && s.url == sub.url)
        {
            return Err("这个地址已添加，请更新已有订阅".into());
        }
        if self.inner.is_shut_down() {
            return Err(SHUT_DOWN.into());
        }
        let base = review::fingerprint(snapshot.subscriptions.iter().find(|s| s.id == sub.id));
        let port = self.inner.subscription_port(&sub).await;
        let downloaded = match download_subscription(&sub, port).await {
            Ok(d) => d,
            Err(e) => {
                self.inner
                    .subscription_failure(&sub, &base, e.to_string())
                    .await;
                return Err(e.to_string());
            }
        };
        let nodes = match self.inner.check_nodes(&sub, downloaded.parsed.nodes).await {
            Ok(nodes) => nodes,
            Err(e) => {
                self.inner
                    .subscription_failure(&sub, &base, e.to_string())
                    .await;
                return Err(e.to_string());
            }
        };
        let old = snapshot.subscriptions.iter().find(|s| s.id == sub.id);
        let (added, removed) =
            review::changes(old.map(|s| s.nodes.as_slice()).unwrap_or_default(), &nodes);
        let affected = review::affected(&snapshot, &sub.id, Some(&removed));
        sub.nodes = nodes;
        sub.info = old.and_then(|s| {
            let mut previous = s.clone();
            review::initialize_info(&mut previous);
            previous.info
        });
        sub.info.get_or_insert_with(Default::default).usage = downloaded.usage;
        Ok(SubscriptionPreview {
            subscription: sub,
            base,
            added,
            removed,
            affected,
            skipped_tailscale: downloaded.parsed.skipped_tailscale,
            via_proxy: downloaded.via_proxy,
        })
    }

    /// Rechecks under the mutation lock so a stale preview cannot overwrite newer work.
    pub async fn commit_subscription_preview(
        &self,
        preview: SubscriptionPreview,
        replacement: Option<NodeRef>,
    ) -> Result<ApplyOutcome, Vec<ValidationError>> {
        let mut eng = self.inner.engine.lock().await;
        let invalid = |what: &str| vec![ValidationError::DanglingRef { what: what.into() }];
        if self.inner.is_shut_down() {
            return Err(invalid("程序正在退出，请稍后重试"));
        }
        let current = eng
            .model
            .subscriptions
            .iter()
            .find(|s| s.id == preview.subscription.id);
        if review::fingerprint(current) != preview.base {
            return Err(invalid("订阅已变化，请重新预览"));
        }
        let (added, removed) = review::changes(
            current.map(|s| s.nodes.as_slice()).unwrap_or_default(),
            &preview.subscription.nodes,
        );
        let mut m = eng.model.clone();
        let id = preview.subscription.id.clone();
        if m.subscriptions
            .iter()
            .any(|s| s.id != id && s.url == preview.subscription.url)
        {
            return Err(invalid("这个地址已添加，请更新已有订阅"));
        }
        let affected = review::affected(&m, &id, Some(&removed));
        let mut sub = preview.subscription;
        sub.last_updated = Some(now_unix());
        sub.last_result = Some(FetchResult::Ok {
            node_count: sub.nodes.len() as u32,
            skipped_tailscale: preview.skipped_tailscale,
        });
        if let Some(s) = m.subscriptions.iter_mut().find(|s| s.id == id) {
            *s = sub;
        } else {
            m.subscriptions.push(sub);
        }
        if !affected.is_empty() {
            let node = replacement.ok_or_else(|| invalid("请先为正在使用的节点选择替代节点"))?;
            if crate::model::expand(&m, &[ExitMember::Node(node.clone())])
                .nodes
                .is_empty()
            {
                return Err(invalid("替代节点不可用，请重新选择"));
            }
            review::replace_references(&mut m, &id, Some(&removed), &node);
        }
        normalize(&mut m);
        validate(&m)?;
        let outcome = self.inner.check_commit_apply(&mut eng, m).await?;
        let mut m = eng.model.clone();
        if let Some(s) = m.subscriptions.iter_mut().find(|s| s.id == id) {
            let result = s.last_result.clone().unwrap();
            review::record(
                s,
                result,
                added,
                removed,
                preview.via_proxy,
                Some(outcome.clone()),
                now_unix(),
            );
        }
        self.inner.commit_model(&mut eng, m);
        self.inner.emit_status();
        Ok(outcome)
    }

    pub async fn delete_subscription_replacing(
        &self,
        id: SubscriptionId,
        replacement: Option<NodeRef>,
    ) -> Result<SubscriptionDeleted, Vec<ValidationError>> {
        let mut eng = self.inner.engine.lock().await;
        if self.inner.is_shut_down() {
            return Err(vec![ValidationError::DanglingRef {
                what: SHUT_DOWN.into(),
            }]);
        }
        let before = eng.model.clone();
        let mut next = before.clone();
        let m = &mut next;
        let used = review::affected(m, &id, None);
        if !used.is_empty() {
            let node = replacement.ok_or_else(|| {
                vec![ValidationError::DanglingRef {
                    what: used.join("、"),
                }]
            })?;
            if matches!(&node, NodeRef::Subscription { subscription_id, .. } if subscription_id == &id)
                || crate::model::expand(m, &[ExitMember::Node(node.clone())])
                    .nodes
                    .is_empty()
            {
                return Err(vec![ValidationError::DanglingRef {
                    what: "请从其他订阅或手动节点选择替代节点".into(),
                }]);
            }
            review::replace_references(m, &id, None, &node);
        }
        m.subscriptions.retain(|s| s.id != id);
        normalize(m);
        validate(m)?;
        let outcome = self.inner.check_commit_apply(&mut eng, next).await?;
        let token = uuid::Uuid::new_v4().to_string();
        *self.inner.subscription_undo.lock().unwrap() = Some(DeletedSnapshot {
            token: token.clone(),
            before,
            after: eng.model.clone(),
            at: Instant::now(),
        });
        Ok(SubscriptionDeleted { token, outcome })
    }

    pub async fn restore_subscription(
        &self,
        token: String,
    ) -> Result<ApplyOutcome, Vec<ValidationError>> {
        let mut eng = self.inner.engine.lock().await;
        let deleted = self.inner.subscription_undo.lock().unwrap().take();
        let invalid = || {
            vec![ValidationError::DanglingRef {
                what: "配置已变化或撤销已过期，为避免覆盖新配置，请重新添加订阅".into(),
            }]
        };
        let deleted = deleted
            .filter(|d| {
                d.token == token && d.at.elapsed() < Duration::from_secs(30) && d.after == eng.model
            })
            .ok_or_else(invalid)?;
        if self.inner.is_shut_down() {
            return Err(invalid());
        }
        self.inner
            .check_commit_apply(&mut eng, deleted.before)
            .await
    }

    pub async fn retry_subscription_apply(
        &self,
        id: SubscriptionId,
    ) -> Result<ApplyOutcome, String> {
        let mut eng = self.inner.engine.lock().await;
        if self.inner.is_shut_down() {
            return Err(SHUT_DOWN.into());
        }
        if !eng
            .model
            .subscriptions
            .iter()
            .any(|s| s.id == id && matches!(s.last_result, Some(FetchResult::Ok { .. })))
        {
            return Err("请先成功下载订阅再重试应用".into());
        }
        let outcome = self.inner.apply(&mut eng).await;
        let mut m = eng.model.clone();
        if let Some(s) = m.subscriptions.iter_mut().find(|s| s.id == id) {
            let result = s.last_result.clone().unwrap();
            review::record(
                s,
                result,
                vec![],
                vec![],
                false,
                Some(outcome.clone()),
                now_unix(),
            );
        }
        self.inner.commit_model(&mut eng, m);
        self.inner.emit_status();
        Ok(outcome)
    }
}

impl Inner {
    async fn subscription_failure(&self, sub: &Subscription, base: &str, reason: String) {
        let mut eng = self.engine.lock().await;
        if self.is_shut_down() {
            return;
        }
        let mut m = eng.model.clone();
        if let Some(s) = m
            .subscriptions
            .iter_mut()
            .find(|s| s.id == sub.id && s.url == sub.url)
        {
            if review::fingerprint(Some(s)) != base {
                return;
            }
            review::initialize_info(s);
            let result = FetchResult::Failed { reason };
            s.last_updated = Some(now_unix());
            s.last_result = Some(result.clone());
            review::record(s, result, vec![], vec![], false, None, now_unix());
            self.commit_model(&mut eng, m);
            self.emit_status();
        }
    }

    async fn subscription_port(&self, sub: &Subscription) -> Option<u16> {
        let mut eng = self.engine.lock().await;
        let pid = eng.core.as_ref()?.pid;
        let Engine { model, alloc, .. } = &mut *eng;
        let fetch = fetch_listener_ports(model, alloc);
        let st = self.status.lock().unwrap();
        if sub
            .fallback_listener
            .as_ref()
            .is_some_and(|id| !st.effective_listeners.contains(id))
        {
            return None;
        }
        let exit = sub.fallback_exit.as_ref()?;
        if !st
            .health
            .exits
            .get(exit)
            .is_some_and(|h| h.status == ExitStatus::Ok)
        {
            return None;
        }
        let port = fetch.iter().find(|(e, _)| e == exit)?.1;
        (self.ports.listening_owners(&[port]).get(&port) == Some(&pid)).then_some(port)
    }
}
