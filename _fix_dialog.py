# -*- coding: utf-8 -*-
import io

p = r'ui\color_picker.slint'
s = io.open(p, encoding='utf-8').read()

# 1) PickerMath：新增 WheelPos 结构 + clamp-wheel 回调（取色点钳制在盘内）
old = '''export global PickerMath {
    pure callback hex-color(string) -> color;              // hex 串 → color（非法回黑）'''
new = '''// 色轮取样点（0..1，已钳制在盘内缘）
export struct WheelPos {
    mx: float,
    my: float,
}

export global PickerMath {
    pure callback hex-color(string) -> color;              // hex 串 → color（非法回黑）'''
assert old in s, 'p1'
s = s.replace(old, new)

old = '''    pure callback fit-hex(string) -> string;               // 守卫调亮预览（fit_contrast）'''
new = '''    pure callback fit-hex(string) -> string;               // 守卫调亮预览（fit_contrast）
    pure callback clamp-wheel(float, float) -> WheelPos;   // 取样点钳制到盘内缘'''
assert old in s, 'p2'
s = s.replace(old, new)

# 2) 色轮：蒙罩圆形化 + 取样点钳制
old = '''                // 明度蒙罩：整盘随明度条压暗（与逐像素缩放 V 等价）
                Rectangle {
                    width: 216px;
                    height: 216px;
                    background: #000000;
                    opacity: 1 - root.pick-v;
                }'''
new = '''                // 明度蒙罩：整盘随明度条压暗（与逐像素缩放 V 等价）；
                // 圆形蒙罩 —— 方形会把圆外的透明角也压暗（背景色不协调的根因）
                Rectangle {
                    width: 216px;
                    height: 216px;
                    border-radius: 108px;
                    background: #000000;
                    opacity: 1 - root.pick-v;
                }'''
assert old in s, 'p3'
s = s.replace(old, new)

old = '''                wheel-ta := TouchArea {
                    mouse-cursor: crosshair;
                    moved => {
                        root.wheel-mx = Math.max(0, Math.min(1, self.mouse-x / wheel-box.width));
                        root.wheel-my = Math.max(0, Math.min(1, self.mouse-y / wheel-box.height));
                        root.hex = PickerMath.wheel-pick(
                            root.wheel-mx * 2 - 1, 1 - root.wheel-my * 2, root.pick-v);
                    }
                    clicked => {
                        root.wheel-mx = Math.max(0, Math.min(1, self.mouse-x / wheel-box.width));
                        root.wheel-my = Math.max(0, Math.min(1, self.mouse-y / wheel-box.height));
                        root.hex = PickerMath.wheel-pick(
                            root.wheel-mx * 2 - 1, 1 - root.wheel-my * 2, root.pick-v);
                    }
                }'''
new = '''                wheel-ta := TouchArea {
                    mouse-cursor: crosshair;
                    moved => {
                        root.wheel-mx = PickerMath.clamp-wheel(
                            self.mouse-x / wheel-box.width, self.mouse-y / wheel-box.height).mx;
                        root.wheel-my = PickerMath.clamp-wheel(
                            self.mouse-x / wheel-box.width, self.mouse-y / wheel-box.height).my;
                        root.hex = PickerMath.wheel-pick(
                            root.wheel-mx * 2 - 1, 1 - root.wheel-my * 2, root.pick-v);
                    }
                    clicked => {
                        root.wheel-mx = PickerMath.clamp-wheel(
                            self.mouse-x / wheel-box.width, self.mouse-y / wheel-box.height).mx;
                        root.wheel-my = PickerMath.clamp-wheel(
                            self.mouse-x / wheel-box.width, self.mouse-y / wheel-box.height).my;
                        root.hex = PickerMath.wheel-pick(
                            root.wheel-mx * 2 - 1, 1 - root.wheel-my * 2, root.pick-v);
                    }
                }'''
assert old in s, 'p4'
s = s.replace(old, new)

# 3) fmt thumb：滑动动效对齐 SegGroup（180ms ease-in-out）
old = '''                        border-radius: 6px;
                        background: PickerMath.hex-color(root.hex);
                        animate background { duration: 120ms; }
                    }'''
new = '''                        border-radius: 6px;
                        background: PickerMath.hex-color(root.hex);
                        animate background { duration: 120ms; }
                        // 滑动动效与「跟随主题/自定义」分段一致
                        animate x { duration: 180ms; easing: ease-in-out; }
                    }'''
assert old in s, 'p5'
s = s.replace(old, new)

# 4) HEX 高度 34 → 32
old = '''                hex-box := Rectangle {
                    height: 34px;'''
new = '''                hex-box := Rectangle {
                    height: 32px;'''
assert old in s, 'p6'
s = s.replace(old, new)

# 5) 对话框 462 → 474
old = '''    width: 462px;
    height: dlg-lay.preferred-height;'''
new = '''    width: 474px;
    height: dlg-lay.preferred-height;'''
assert old in s, 'p7'
s = s.replace(old, new)

# 6) 示例条三列统一（26px 内容区垂直居中 + 标签；链接不再偏移）
i = s.index('            VerticalLayout {\n                alignment: end;\n                spacing: 5px;\n                horizontal-stretch: 1;\n                HorizontalLayout {')
j = s.index('        // ── 守卫提示：低对比度预告调亮结果 + 写回说明 ────────────────────')
new = '''            VerticalLayout {
                spacing: 5px;
                horizontal-stretch: 1;
                HorizontalLayout {
                    height: 26px;
                    alignment: center;
                    spacing: 6px;
                    Rectangle {
                        y: (parent.height - self.height) / 2;
                        width: 8px;
                        height: 8px;
                        border-radius: 3px;
                        background: PickerMath.hex-color(root.accent-pick-hex);
                    }
                    Text {
                        vertical-alignment: center;
                        text: root.lang-en ? "Session" : "会话";
                        color: PickerMath.hex-color(root.accent-pick-hex);
                        font-size: 11.5px;
                        font-weight: 500;
                    }
                }
                Text {
                    text: root.lang-en ? "Selection" : "选中态";
                    color: Theme.text-muted;
                    font-size: 10px;
                    horizontal-alignment: center;
                }
            }
            VerticalLayout {
                spacing: 5px;
                horizontal-stretch: 1;
                Rectangle {
                    height: 26px;
                    border-radius: 6px;
                    background: PickerMath.hex-color(root.accent-pick-hex);
                    Text {
                        width: 100%;
                        height: 100%;
                        text: root.lang-en ? "Connect" : "连接";
                        color: PickerMath.ink-on(root.accent-pick-hex);
                        font-size: 11.5px;
                        font-weight: 600;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                }
                Text {
                    text: root.lang-en ? "Primary" : "主按钮";
                    color: Theme.text-muted;
                    font-size: 10px;
                    horizontal-alignment: center;
                }
            }
            VerticalLayout {
                spacing: 5px;
                horizontal-stretch: 1;
                HorizontalLayout {
                    height: 26px;
                    alignment: center;
                    Text {
                        text: root.lang-en ? "Docs" : "查看文档";
                        color: PickerMath.hex-color(root.accent-pick-hex);
                        font-size: 11.5px;
                    }
                }
                Text {
                    text: root.lang-en ? "Link" : "链接";
                    color: Theme.text-muted;
                    font-size: 10px;
                    horizontal-alignment: center;
                }
            }
'''
s = s[:i] + new + s[j:]

# 7) 取消/确认悬停
old = '''            Rectangle {
                width: 84px;
                height: 30px;
                border-radius: 8px;
                background: transparent;
                border-width: 1px;
                border-color: Theme.border-strong;
                Text {
                    text: root.en ? "Cancel" : "取消";
                    color: Theme.text-secondary;
                    font-size: 12px;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
                TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.cancelled(); }
                }
            }
            Rectangle {
                width: 84px;
                height: 30px;
                border-radius: 8px;
                background: PickerMath.hex-color(root.hex);
                Text {
                    text: root.en ? "Confirm" : "确认";
                    color: PickerMath.ink-on(root.hex);
                    font-size: 12px;
                    font-weight: 600;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
                TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.confirmed(); }
                }
            }'''
new = '''            Rectangle {
                width: 84px;
                height: 30px;
                border-radius: 8px;
                background: cancel-ta.has-hover ? Theme.bg-hover : transparent;
                border-width: 1px;
                border-color: cancel-ta.has-hover ? Theme.text-muted : Theme.border-strong;
                Text {
                    text: root.en ? "Cancel" : "取消";
                    color: cancel-ta.has-hover ? Theme.text-primary : Theme.text-secondary;
                    font-size: 12px;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
                cancel-ta := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.cancelled(); }
                }
            }
            Rectangle {
                width: 84px;
                height: 30px;
                border-radius: 8px;
                background: PickerMath.hex-color(root.hex);
                clip: true;
                // 悬停提亮 + 按下压暗
                Rectangle {
                    background: #ffffff;
                    opacity: ok-ta.has-hover ? 0.12 : ok-ta.pressed ? 0.22 : 0;
                }
                Text {
                    text: root.en ? "Confirm" : "确认";
                    color: PickerMath.ink-on(root.hex);
                    font-size: 12px;
                    font-weight: 600;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
                ok-ta := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.confirmed(); }
                }
            }'''
assert old in s, 'p8'
s = s.replace(old, new)

io.open(p, 'w', encoding='utf-8').write(s)
print('dialog fixed')
