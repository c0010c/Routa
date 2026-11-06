//! Idempotent legacy shared-outbound migration. Original entries are kept for
//! subscription-only references and desktop preferences until they are updated.
use super::*;
pub fn private_outbounds(model: &mut Model) {
    model.settings.core_path = None;
    let shared: Vec<ExitId> = model.exits.iter().filter(|e| model.listeners.iter().filter(|l| l.exit.as_ref()==Some(&e.id)).count()>1).map(|e|e.id.clone()).collect();
    for i in 0..model.listeners.len() {
        let l = &model.listeners[i];
        let Some(old) = l.exit.as_ref().and_then(|id| model.exits.iter().find(|e| &e.id == id)).cloned() else { continue };
        let id = ExitId(format!("proxy-{}", l.id.0));
        if old.id == id { continue; }
        if !shared.contains(&old.id) { continue; }
        let mut owned = old.clone();
        owned.id = id.clone();
        owned.name = format!("代理-{}", l.id.0);
        model.listeners[i].exit = Some(id.clone());
        if !model.exits.iter().any(|e| e.id == id) { model.exits.push(owned); }
        for sub in &mut model.subscriptions {
            if sub.fallback_exit.as_ref() == Some(&old.id) { sub.fallback_exit = Some(id.clone()); }
        }
    }
    model.exits.retain(|e| !shared.contains(&e.id) || model.listeners.iter().any(|l| l.exit.as_ref()==Some(&e.id)) || model.subscriptions.iter().any(|s| s.fallback_exit.as_ref()==Some(&e.id)));
    // Preserve standalone legacy configurations as disabled proxies, including
    // subscription-only fallbacks. They remain editable without opening a shared manager.
    let orphans: Vec<Exit> = model.exits.iter().filter(|e| !model.listeners.iter().any(|l| l.exit.as_ref()==Some(&e.id))).cloned().collect();
    for e in orphans {
        let port = (7881..=65535).find(|port| *port != model.settings.controller_port && !model.listeners.iter().any(|l| l.port==*port));
        let Some(port) = port else { continue; };
        let mut name = e.name.clone();
        for n in 2.. { if !model.listeners.iter().any(|l| l.name==name) { break; } name = format!("{} {n}", e.name); }
        let mut l = Listener::new_default(name, port);
        l.listen = "127.0.0.1".into(); l.user_enabled = false; l.exit = Some(e.id);
        model.listeners.push(l);
    }
    for sub in &mut model.subscriptions {
        if sub.fallback_listener.is_none() { sub.fallback_listener = model.listeners.iter().find(|l| l.exit.is_some() && l.exit == sub.fallback_exit).map(|l| l.id.clone()); }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_outbound_copied_without_losing_strategy_or_interface() {
        let mut m = Model::default();
        let e = Exit { name: "legacy".into(), interface: Some(InterfaceGuid("nic".into())), ..Exit::default() };
        for port in [7881, 7882] {
            let mut l = Listener::new_default(format!("p{port}"), port);
            l.exit = Some(e.id.clone()); m.listeners.push(l);
        }
        m.exits.push(e.clone());
        private_outbounds(&mut m);
        assert_ne!(m.listeners[0].exit, m.listeners[1].exit);
        for l in &m.listeners {
            let owned = m.exits.iter().find(|e| Some(&e.id) == l.exit.as_ref()).unwrap();
            assert_eq!(owned.kind, e.kind); assert_eq!(owned.interface, e.interface);
        }
        let before = m.clone(); private_outbounds(&mut m); assert_eq!(before, m);
    }
}
