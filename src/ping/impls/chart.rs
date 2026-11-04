//! RTT 曲线的 SVG path 拼接（主图 320×74 / 清单行迷你趋势 40×12）。

/// 图表视口（与 ping_dialog.slint 的 chart 区域一致）。
const CHART_W: f64 = 320.0;
const CHART_H: f64 = 74.0;
/// 迷你趋势视口（清单行）。
const SPARK_W: f64 = 40.0;
const SPARK_H: f64 = 12.0;

/// 主曲线 + 区域填充；None 点断段。返回 (line, area)。
pub(crate) fn chart_paths(samples: &[Option<f64>]) -> (String, String) {
    let valid: Vec<f64> = samples.iter().flatten().copied().collect();
    if valid.is_empty() { return (String::new(), String::new()); }
    let max_v = valid.iter().cloned().fold(0.0f64, f64::max);
    let scale = (max_v * 1.25 / 10.0).ceil() * 10.0;
    let scale = scale.max(40.0);
    let n = samples.len().max(2);
    let x_at = |i: usize| (i as f64 / (n - 1) as f64) * CHART_W;
    let y_at = |v: f64| CHART_H - 3.0 - (v / scale) * (CHART_H - 6.0);
    let mut line = String::new();
    let mut area = String::new();
    let mut pen = false;
    let mut seg_start: Option<usize> = None;
    let mut seg_pts: Vec<(f64, f64)> = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        match s {
            Some(v) => {
                if !pen { seg_start = Some(i); seg_pts.clear(); }
                let (x, y) = (x_at(i), y_at(*v));
                line.push_str(if pen { " L " } else { "M " });
                line.push_str(&format!("{:.1} {:.1}", x, y));
                seg_pts.push((x, y));
                pen = true;
            }
            None => {
                if pen {
                    // 断段：闭合当前段的区域
                    if let Some(s0) = seg_start {
                        area.push_str(&area_seg(&seg_pts, x_at(s0), x_at(i.saturating_sub(1)), CHART_H - 3.0));
                    }
                    pen = false;
                }
            }
        }
    }
    if pen {
        if let Some(s0) = seg_start {
            area.push_str(&area_seg(&seg_pts, x_at(s0), x_at(samples.len() - 1), CHART_H - 3.0));
        }
    }
    (line, area)
}

fn area_seg(pts: &[(f64, f64)], x0: f64, x1: f64, base: f64) -> String {
    if pts.is_empty() { return String::new(); }
    let mut d = String::from("M ");
    d.push_str(&format!("{:.1} {:.1} L ", x0, base));
    for (x, y) in pts { d.push_str(&format!("{:.1} {:.1} L ", x, y)); }
    d.push_str(&format!("{:.1} {:.1} Z ", x1, base));
    d
}

/// 丢包刻度：None 点在底部的红色短竖线。
pub(crate) fn loss_path(samples: &[Option<f64>]) -> String {
    let n = samples.len().max(2);
    let mut d = String::new();
    for (i, s) in samples.iter().enumerate() {
        if s.is_none() {
            let x = (i as f64 / (n - 1) as f64) * CHART_W;
            d.push_str(&format!("M {:.1} {:.1} L {:.1} {:.1} ", x, CHART_H - 6.0, x, CHART_H));
        }
    }
    d
}

/// 行迷你趋势（40×12，min-max 归一）。
pub(crate) fn spark_path(samples: &[Option<f64>]) -> String {
    let valid: Vec<f64> = samples.iter().flatten().copied().collect();
    if valid.len() < 2 { return String::new(); }
    let max = valid.iter().cloned().fold(f64::MIN, f64::max);
    let min = valid.iter().cloned().fold(f64::MAX, f64::min);
    let span = (max - min).max(1.0);
    let n = samples.len();
    let mut d = String::new();
    let mut pen = false;
    for (i, s) in samples.iter().enumerate() {
        match s {
            Some(v) => {
                let x = (i as f64 / (n - 1).max(1) as f64) * SPARK_W;
                let y = SPARK_H - ((v - min) / span) * (SPARK_H - 2.0) - 1.0;
                d.push_str(if pen { " L " } else { "M " });
                d.push_str(&format!("{:.1} {:.1}", x, y));
                pen = true;
            }
            None => pen = false,
        }
    }
    d
}
