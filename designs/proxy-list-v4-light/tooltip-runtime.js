// Standalone preview support. Inline previews use the host's data-tooltip behavior.
(()=>{
  const root=document.getElementById('routa-list-prototype');
  if(!root)return;
  const tip=document.createElement('div');
  tip.id='l-inline-help';
  tip.className='l-prototype-tooltip';
  tip.setAttribute('role','tooltip');
  tip.setAttribute('popover','manual');
  tip.hidden=true;
  root.append(tip);
  let anchor=null,pinned=false,showTimer,hideTimer,previousDescription=null;
  const nativePopover=typeof tip.showPopover==='function';
  const triggerFor=target=>target instanceof Element?target.closest('#routa-list-prototype [data-tooltip]'):null;
  const visible=()=>!tip.hidden;
  function hide(){
    clearTimeout(showTimer);clearTimeout(hideTimer);
    if(nativePopover&&tip.matches(':popover-open'))tip.hidePopover();
    tip.hidden=true;
    if(anchor){
      if(previousDescription===null)anchor.removeAttribute('aria-describedby');
      else anchor.setAttribute('aria-describedby',previousDescription);
    }
    anchor=null;pinned=false;previousDescription=null;
  }
  function position(){
    if(!anchor?.isConnected){hide();return;}
    const r=anchor.getBoundingClientRect(),t=tip.getBoundingClientRect(),gap=8,pad=12;
    const width=document.documentElement.clientWidth,height=window.innerHeight;
    tip.style.left=Math.max(pad,Math.min(r.left+r.width/2-t.width/2,width-t.width-pad))+'px';
    let top=r.bottom+gap;
    if(top+t.height>height-pad)top=r.top-t.height-gap;
    tip.style.top=Math.max(pad,Math.min(top,height-t.height-pad))+'px';
  }
  function show(trigger,pin=false){
    clearTimeout(showTimer);clearTimeout(hideTimer);
    if(!trigger?.isConnected)return;
    if(anchor===trigger&&visible()){pinned=pinned||pin;return;}
    hide();
    anchor=trigger;pinned=pin;previousDescription=trigger.getAttribute('aria-describedby');
    tip.textContent=trigger.dataset.tooltip;
    const container=trigger.closest('dialog[open]')||root;
    if(tip.parentNode!==container)container.append(tip);
    trigger.setAttribute('aria-describedby',[previousDescription,tip.id].filter(Boolean).join(' '));
    tip.hidden=false;
    if(nativePopover)tip.showPopover();
    position();
  }
  function hideAfterPointerLeave(){
    clearTimeout(showTimer);clearTimeout(hideTimer);
    hideTimer=setTimeout(()=>{
      if(!pinned&&document.activeElement!==anchor&&!anchor?.matches(':hover')&&!tip.matches(':hover'))hide();
    },140);
  }
  document.addEventListener('pointerover',event=>{
    const trigger=triggerFor(event.target);
    if(trigger&&!trigger.contains(event.relatedTarget)){
      clearTimeout(hideTimer);clearTimeout(showTimer);
      showTimer=setTimeout(()=>show(trigger),250);
    }
    if(tip.contains(event.target))clearTimeout(hideTimer);
  });
  document.addEventListener('pointerout',event=>{
    if(triggerFor(event.target)||tip.contains(event.target))hideAfterPointerLeave();
  });
  document.addEventListener('focusin',event=>{
    const trigger=triggerFor(event.target);
    if(trigger)show(trigger);
    else if(!tip.contains(event.target))hide();
  });
  document.addEventListener('focusout',event=>{
    if(event.target===anchor){pinned=false;hideAfterPointerLeave();}
  });
  document.addEventListener('pointerdown',event=>{
    if(visible()&&!tip.contains(event.target)&&triggerFor(event.target)!==anchor)hide();
  },true);
  document.addEventListener('click',event=>{
    const trigger=triggerFor(event.target);
    if(!trigger)return;
    if(anchor===trigger&&pinned)hide();
    else show(trigger,true);
  });
  document.addEventListener('keydown',event=>{
    if(event.key==='Escape'&&visible()){
      event.preventDefault();event.stopPropagation();hide();
    }
  },true);
  document.addEventListener('scroll',hide,true);
  window.addEventListener('resize',hide);
  new MutationObserver(()=>{if(anchor&&!anchor.isConnected)hide();}).observe(root,{childList:true,subtree:true});
})();
