def lin(c):
    c=c/255
    return c/12.92 if c<=0.04045 else ((c+0.055)/1.055)**2.4
def L(h):
    h=h.lstrip('#')
    r,g,b=[int(h[i:i+2],16) for i in (0,2,4)]
    return 0.2126*lin(r)+0.7152*lin(g)+0.0722*lin(b)
def cr(a,b):
    la,lb=L(a),L(b)
    hi,lo=max(la,lb),min(la,lb)
    return (hi+0.05)/(lo+0.05)
def over(fg,bg,alpha):
    fg=fg.lstrip('#'); bg=bg.lstrip('#')
    o=[]
    for i in (0,2,4):
        f=int(fg[i:i+2],16); b=int(bg[i:i+2],16)
        o.append(round(alpha*f+(1-alpha)*b))
    return '#%02X%02X%02X'%tuple(o)

pairs=[
 ("dark  .ml/.tag/.meta  t3 on panel","#6F737A","#181A1D"),
 ("dark  .ml on selected ac-tonal","#7A7E85",over("#1FC8B4","#181A1D",.10)),
 ("dark  .ds  t2 on panel","#7A7E85","#181A1D"),
 ("dark  .sig t1 on panel","#BCBEC4","#181A1D"),
 ("dark  .wt  dg on dg-soft","#F75464",over("#F75464","#181A1D",.13)),
 ("dark  ghost t3@.62 on tbg","#6F737A","#181A1D"),
 ("light .ml/.tag/.meta  t3 on panel","#968F85","#FAF8F4"),
 ("light .ds  t2 on panel","#7B766F","#FAF8F4"),
 ("dark  .ml t3 on elev","#6F737A","#1E2024"),
 ("light .ml t3 on elev","#968F85","#FFFEFA"),
]
for name,fg,bg in pairs:
    if name.endswith("on tbg") or "ghost" in name:
        fg2=over(fg,bg,.62)
        print(f"{name:42s} fg={fg2} bg={bg} ratio={cr(fg2,bg):.2f}")
    else:
        print(f"{name:42s} fg={fg} bg={bg} ratio={cr(fg,bg):.2f}")
print()
print("ghost light:", over("#968F85","#FAF8F4",.62), f"{cr(over('#968F85','#FAF8F4',.62),'#FAF8F4'):.2f}")
