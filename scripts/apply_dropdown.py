# -*- coding: utf-8 -*-
"""把 theme-wallpaper-split 的主题选择改成「下拉框 + 悬停预览」。"""

import io
import re

from gen_split_v2 import ALL, GROUPS_ORD, ALL_IDS, NEW_CSS, vars_css, js_data

P = '../docs/design/theme-wallpaper-split-2026-10-05.html'
s = io.open(P, encoding='utf-8').read()

# ══════════════════════════════════════════════════════════════════
# ① CSS：替换 33 套变量块 + 追加下拉/预览样式
# ══════════════════════════════════════════════════════════════════
i0 = s.index('  /* 德古拉 · 暗 —— 23598★ */')
i1 = s.index('\n  /* ── 真实界面', i0) if '\n  /* ── 真实界面' in s[i0:] \
    else s.index('  .demo{', i0)
s = s[:i0] + vars_css() + s[i1:]

# 追加下拉/预览 CSS（放在 style 末尾）
anchor = '</style>'
assert anchor in s
s = s.replace(anchor, NEW_CSS + '\n' + anchor, 1)

# ══════════════════════════════════════════════════════════════════
# ② 设置页：网格 → 下拉 + 预览
# ══════════════════════════════════════════════════════════════════
old_pick = s[s.index('<div class="tpick" style="margin-top:8px" id="pick"></div>')
              -260:s.index('<div class="tpick" style="margin-top:8px" id="pick"></div>')
              + len('<div class="tpick" style="margin-top:8px" id="pick"></div>')]
NEW_PICK = '''<div class="tpk" style="margin-top:9px">
                  <div class="tpk-btn" id="tpkBtn">
                    <div class="tpk-cur">
                      <span class="tpk-thumb" id="tpkThumb"></span>
                      <span class="tpk-txt">
                        <span class="tpk-n1" id="tpkCurN1">—</span>
                        <span class="tpk-n2" id="tpkCurN2">—</span>
                      </span>
                      <span class="tpk-caret">▼</span>
                    </div>
                    <div class="tpk-pop">
                      <div class="tpk-search">
                        <input class="tpk-si" id="tpkSearch" placeholder="搜索主题…">
                      </div>
                      <div class="tpk-list" id="tpkList"></div>
                    </div>
                  </div>
                  <div class="tpk-prev" id="tpkPrev"></div>
                </div>'''
s = s.replace(old_pick, NEW_PICK, 1)

# ══════════════════════════════════════════════════════════════════
# ③ 顶栏弹层：网格 → 紧凑列表（空间小，缩略图更小）
# ══════════════════════════════════════════════════════════════════
s = s.replace(
    '''<div style="display:grid;grid-template-columns:repeat(4,1fr);gap:6px" id="pop"></div>''',
    '''<div id="pop"></div>''')

# ══════════════════════════════════════════════════════════════════
# ④ JS 全换
# ══════════════════════════════════════════════════════════════════
NEW_JS = js_data() + r'''
/* ── 文档区色（不随主题变）────────────────────────────────────
   说明这张页面自己的配色。与主题变量是**两套独立定义** ——
   否则切主题时连对照表本身都变色，读不了。 */
var DOC={bg:'#1b1d23',panel:'#23262d',alt:'#2a2d35',line:'#3a3d46',
  line2:'#4a4e59',t1:'#e6e8ee',t2:'#b4b9c4',t3:'#9196a3',
  ac:'#4a90e2',ok:'#4ec9b0',wr:'#e2a84a',dg:'#e25c5c',hov:'#2e323a'};
function docVars(){var r=document.documentElement.style;
  for(var k in DOC) r.setProperty('--doc-'+k,DOC[k]);}

/* ── 强调色三态 ───────────────────────────────────────────────
   ★ 用户需求：强调色**没设置时跟随主题色**。
   实现：主题的 accent 写在 [data-theme] 里，用户覆盖写在 :root
   （更高优先级，但**只在覆盖时才出现**）。follow = 移除覆盖。 */
var accentState={mode:'follow',value:null}, curId=null;

var PRESETS=[
  {n:'跟随主题',v:null},
  {n:'经典蓝',   v:'#0090ff',from:'graphite-dark'},
  {n:'青',v:'#00a2c7',from:'slate-dark'},
  {n:'靛紫',     v:'#5b5bd6',from:'mauve-dark'},
  {n:'翡翠',     v:'#29a383',from:'sage-dark'},
  {n:'琥珀',     v:'#ffc53d',from:'sand-dark'},
  {n:'番茄',     v:'#e54d2e',from:'olive-dark'},
  {n:'霓蓝',     v:'#7aa2f7',from:'tokyonight-dark'},
  {n:'霜青',     v:'#88c0d0',from:'nord-dark'},
  {n:'暮紫',     v:'#bd93f9',from:'dracula-dark'}
];

function hexA(h,a){var n=parseInt(h.slice(1),16);
  return'rgba('+((n>>16)&255)+','+((n>>8)&255)+','+(n&255)+','+a+')';}

function applyAccent(){
  var r=document.documentElement.style;
  /* ⚠️ setProperty / removeProperty 是 CSSStyleDeclaration 的方法，
     不是 Element 的 —— 直接在 element 上调会抛 is not a function。 */
  if(accentState.mode==='follow'){
    /* ★ 回退的关键：**移除**覆盖属性，而不是设成空串。
       空串会让 var(--accent) 解析失败 → 整条继承链断掉。 */
    ['--accent','--accent-soft','--accent-soft-hi',
     '--accent-chip','--accent-line'].forEach(function(k){
      r.removeProperty(k);
    });
  }else{
    var c=accentState.value;
    r.setProperty('--accent',c);
    r.setProperty('--accent-soft',    hexA(c,.15));
    r.setProperty('--accent-soft-hi', hexA(c,.24));
    r.setProperty('--accent-chip',    hexA(c,.22));
    r.setProperty('--accent-line',    hexA(c,.34));
  }
  document.querySelectorAll('.ac').forEach(function(e){
    var v=e.getAttribute('data-v');
    e.classList.toggle('on',
      (accentState.mode==='follow'&&v==='')||
      (accentState.mode!=='follow'&&v===accentState.value));
  });
  var st=document.getElementById('accState');
  if(st) st.textContent = accentState.mode==='follow'
    ? '跟随主题 · 当前生效 '+A()+'（来自'+PAL[curId].f+' · '+PAL[curId].m+'）'
    : '已自定义 · '+accentState.value;
}

var demoEl=null;
function A(){ return demoEl
  ? getComputedStyle(demoEl).getPropertyValue('--accent').trim() : ''; }

/* ── 缩略图（触发器 / 列表行 共用）── */
function thumb(t,w,h){
  return '<span class="tpk-th" style="width:'+w+'px;height:'+h+'px;'+
    'flex-basis:'+w+'px;background:'+t.root+'">'+
    '<i style="position:absolute;left:0;top:0;right:0;height:22%;background:'+
      t.panel+'"></i>'+
    '<i style="position:absolute;left:0;top:22%;right:0;height:26%;background:'+
      t.tbg+'"></i>'+
    '<i style="position:absolute;left:5%;top:30%;width:34%;height:10%;'+
      'background:'+t.ac+';border-radius:1px"></i>'+
    '<i style="position:absolute;left:5%;bottom:8%;width:52%;height:8%;'+
      'background:'+t.t2+';opacity:.75;border-radius:1px"></i></span>';
}

/* ── 预览面板（右侧固定位）── */
function renderPrev(id){
  var t=PAL[id], box=document.getElementById('tpkPrev');
  var star=t.star?('<span class="tpk-pv-star">'+t.star+'★</span>'):'';
  var slots=[['bg-root',t.root],['bg-panel',t.panel],['bg-tab',t.tab],
             ['line',t.line],['text-primary',t.t1],['text-secondary',t.t2],
             ['term-bg',t.tbg],['accent',t.ac]];
  box.innerHTML =
    '<div class="tpk-pv-hd"><span class="tpk-pv-n1">'+t.f+' · '+t.m+'</span>'+
    '<span class="tpk-pv-n2">'+t.en+'</span>'+star+'</div>'+
    '<div class="tpk-pv-bar">'+
      [t.root,t.panel,t.palt,t.elev,t.tab,t.tbg].map(function(c){
        return '<i style="background:'+c+'"></i>';}).join('')+
    '</div>'+
    /* 迷你界面：结构与真实界面同构造（标题栏 + 标签行 + 侧栏 + 终端） */
    '<div class="pv" style="background:'+t.tbg+';color:'+t.tfg+'">'+
      '<div class="pv-bar" style="background:'+t.panel+
        ';border-bottom:1px solid '+t.line+'">'+
        '<i style="background:'+t.dg+'"></i><i style="background:'+t.wr+
        '"></i><i style="background:'+t.ok+'"></i></div>'+
      '<div class="pv-tab" style="background:'+t.tbg+'">'+
        '<span class="pt" style="background:'+t.tab+';color:'+t.t1+'">'+
          '<i class="ltr" style="background:'+t.ac+'"></i>'+
          '<i class="lb" style="color:'+t.ch[0]+'">A</i>'+
          '<i class="tx">ssh root@prod-01</i></span>'+
        '<span class="pt on" style="background:'+t.tab+';color:'+t.t1+'">'+
          '<i class="ltr" style="background:'+t.ac+'"></i>'+
          '<i class="lb" style="color:'+t.ch[1]+'">B</i>'+
          '<i class="tx">deploy · tail -f</i></span>'+
        '<span class="add" style="color:'+t.t3+'">＋</span></div>'+
      '<div style="flex:1;display:flex;min-height:0">'+
        '<div class="pv-side" style="background:'+t.palt+'">'+
          [0,1,2,3,4].map(function(i){return '<i class="'+(i===1?'on':'')+
            '" style="background:'+(i===1?t.ac:t.t3)+
            ';opacity:'+(i===1?1:.5)+'"></i>';}).join('')+'</div>'+
        '<div class="pv-main">'+
          '<i class="ln" style="background:'+t.t2+';width:74%"></i>'+
          '<i class="ln" style="background:'+t.t3+';width:52%"></i>'+
          '<i class="ln" style="background:'+t.ok+';width:38%"></i>'+
          '<i class="mk" style="background:'+t.elev+';width:92%"></i>'+
        '</div></div></div>'+
    '<div class="tpk-pv-desc">'+t.desc+'</div>'+
    '<div class="tpk-slots">'+slots.map(function(s){
      return '<div class="tpk-sl"><span class="sq" style="background:'+s[1]+
        '"></span><b>'+s[0]+'</b>'+s[1]+'</div>';}).join('')+'</div>'+
    '<div class="tpk-pv-lic">授权：'+(t.lic||'—')+
      (t.repo?'<br><span style="color:var(--doc-t3)">'+t.repo+'</span>':'')+'</div>';
}

/* ── 下拉列表（按族分组 + 搜索）── */
var filterKw='';
function renderList(){
  var box=document.getElementById('tpkList');
  var html='', hit=0;
  GROUPS.forEach(function(g){
    var ids=g[3].filter(function(id){
      if(!filterKw) return true;
      var t=PAL[id];
      return (t.f+t.en+t.m+t.repo).toLowerCase().indexOf(filterKw)>=0;
    });
    if(!ids.length) return;
    hit+=ids.length;
    html+='<div class="tpk-grp">'+g[1]+' · '+ids.length+'</div>';
    html+=ids.map(function(id){
      var t=PAL[id];
      return '<div class="tpk-row'+(id===curId?' on':'')+'" data-id="'+id+'">'+
        thumb(t,34,22)+
        '<span class="tpk-nm">'+t.f+' · '+t.m+'</span>'+
        '<span class="tpk-r2">'+(t.star?t.star+'★':t.en.slice(0,6))+'</span>'+
        '</div>';
    }).join('');
  });
  box.innerHTML = hit? html
    : '<div class="tpk-empty">没有匹配「'+filterKw+'」的主题</div>';
  box.querySelectorAll('.tpk-row').forEach(function(r){
    /* 悬停 = 预览（**不切换**，切换只在点击）——
       判据：**悬停是"看"，点击才是"选"**。
       悬停就切换会让用户无法"扫一眼列表找某个主题"。 */
    r.addEventListener('mouseenter', function(){
      var id=r.getAttribute('data-id');
      renderPrev(id);
      hoverId=id;
      if(!pickerOpen) sel(id,{silent:true});
    });
    r.addEventListener('click', function(){
      sel(r.getAttribute('data-id'));
      closePicker();
    });
  });
}

var hoverId=null, pickerOpen=false;

/* ── 选主题 ── */
function sel(id,opt){
  opt=opt||{};
  curId=id;
  document.documentElement.setAttribute('data-theme',id);
  /* ⚠️ 换主题时**先清掉强调色覆盖** —— 自定义色在上一套底色上
     可能刚好达标，换一套就未必。判据：任何由某个底色推导出来的值，
     换底色都要重算。 */
  if(!opt.silent){ accentState.mode='follow'; accentState.value=null; }
  var t=PAL[id];

  /* 演示区（顶栏那条）跟随主题 */
  if(demoEl){
    demoEl.style.background=t.root; demoEl.style.borderColor=t.line;
    var bar=demoEl.querySelector('.dm-bar');
    if(bar){ bar.style.background=t.panel;
      bar.style.color=t.t1; bar.style.borderBottom='1px solid '+t.line; }
    var tb=document.getElementById('dTab');
    if(tb){ tb.style.background=t.tab; tb.style.color=t.t1; }
    var nm=document.getElementById('dName');
    if(nm){ nm.textContent=t.f+' · '+t.m; nm.style.color=t.t2; }
    var tm=document.getElementById('dTerm');
    if(tm){ tm.style.background=t.tbg; tm.style.color=t.tfg;
      tm.style.setProperty('--t-accent',t.ac);
      tm.style.setProperty('--t-accent2',t.ac2);
      tm.style.setProperty('--t-ok',t.ok);
      tm.style.setProperty('--t-warn',t.wr);
      tm.style.setProperty('--t-danger',t.dg);
      tm.style.setProperty('--t-mut',t.t3); }
  }

  /* 触发器当前项 */
  var th=document.getElementById('tpkThumb');
  if(th) th.outerHTML=thumb(t,44,30).replace('tpk-th','tpk-thumb');
  var n1=document.getElementById('tpkCurN1'), n2=document.getElementById('tpkCurN2');
  if(n1) n1.textContent=t.f+' · '+t.m;
  if(n2) n2.textContent=t.en+(t.star?' · '+t.star+'★':'')+
    ' · '+t.group;

  renderPrev(id);
  renderList();
  applyAccent();
  return t;
}

/* ── 开合 ── */
function openPicker(){ pickerOpen=true;
  document.getElementById('tpkBtn').classList.add('open'); }
function closePicker(){ pickerOpen=false;
  document.getElementById('tpkBtn').classList.remove('open');
  /* 关闭时把主题**还原成已选中的那个** ——
     悬停期间预览过别的，不点就等于没选。 */
  if(hoverId && hoverId!==curId) sel(curId);
}

/* ── 顶栏弹层（紧凑：窄空间用更小的缩略图）── */
document.getElementById('pop').innerHTML=GROUPS.map(function(g){
  return '<div class="tpk-grp" style="padding:7px 2px 3px">'+g[1]+'</div>'+
    g[3].map(function(id){
      var t=PAL[id];
      return '<div class="tpk-row" data-id="'+id+'" style="padding:4px 5px">'+
        thumb(t,26,16)+
        '<span class="tpk-nm" style="font-size:11px">'+t.f+' · '+t.m+'</span>'+
        '<span class="tpk-r2">'+(t.star?t.star+'★':'')+'</span></div>';
    }).join('');
}).join('');
document.getElementById('pop').style.maxHeight='340px';
document.getElementById('pop').style.overflowY='auto';
document.getElementById('pop').querySelectorAll('.tpk-row').forEach(function(r){
  r.addEventListener('click',function(){
    sel(r.getAttribute('data-id'));
    document.getElementById('pop').parentNode.style.display='none';
  });
});

/* ── 强调色色点 ── */
function accentHtml(){
  return PRESETS.map(function(p,i){
    var mark=p.v?'':'background:repeating-linear-gradient(45deg,#3a3d46 0 4px,#4a4e59 4px 8px)';
    var fromN=(p.from&&PAL[p.from])?PAL[p.from].f:null;
    return '<div class="ac'+(i===0?' on':'')+'" data-v="'+(p.v||'')+
      '" title="'+p.n+(fromN?'（出自'+fromN+'）':' · 默认')+'" style="'+
      (p.v?'background:'+p.v:'')+'"><i style="'+mark+'"></i></div>';
  }).join('');
}
document.querySelectorAll('.accents').forEach(function(g){
  g.innerHTML=accentHtml();
  g.addEventListener('click',function(ev){
    var e=ev.target.closest('.ac'); if(!e) return;
    var v=e.getAttribute('data-v');
    accentState.mode = v===''?'follow':'custom';
    accentState.value = v===''?null:v;
    applyAccent();
  });
});

/* ── 绑定 ── */
document.getElementById('tpkBtn').addEventListener('click',function(e){
  e.stopPropagation();
  pickerOpen?closePicker():openPicker();
});
document.getElementById('tpkSearch').addEventListener('input',function(e){
  e.stopPropagation();
  filterKw=e.target.value.trim().toLowerCase();
  renderList();
});
document.getElementById('tpkSearch').addEventListener('click',function(e){
  e.stopPropagation();
});
document.addEventListener('click',function(){ if(pickerOpen) closePicker(); });

demoEl=document.getElementById('demo');
docVars();
sel(ALL_IDS[0]);
'''
i0 = s.index('<script>')
i1 = s.index('</script>') + len('</script>')
s = s[:i0] + '<script>\n' + NEW_JS + '\n</script>' + s[i1:]

io.open(P, 'w', encoding='utf-8', newline='').write(s)
print('下拉框 + 悬停预览已写入 · %d 个变体 / 3 族' % len(ALL_IDS))
print('文件 %d 字节' % len(s))
