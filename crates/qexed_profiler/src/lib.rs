//! Qexed sampling profiler: spans, entities, HTML report with tree/flame/map.

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

// --- SpanStats ---
#[derive(Debug, Clone, Default)]
pub struct SpanStats {
    pub name: String,
    pub count: u64,
    pub total_ns: u64,
    pub min_ns: u64,
    pub max_ns: u64,
    samples: Vec<u64>,
    sample_times: Vec<u64>,
}
const MAX_SAMPLES: usize = 2000;

impl SpanStats {
    fn record(&mut self, ns: u64, ts_ms: u64) {
        self.count += 1;
        self.total_ns = self.total_ns.saturating_add(ns);
        if self.count == 1 {
            self.min_ns = ns;
            self.max_ns = ns;
        } else {
            self.min_ns = self.min_ns.min(ns);
            self.max_ns = self.max_ns.max(ns);
        }
        if self.samples.len() < MAX_SAMPLES {
            self.samples.push(ns);
            self.sample_times.push(ts_ms);
        }
    }
    pub fn avg_ns(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.total_ns / self.count
        }
    }
    pub fn total_ms(&self) -> f64 {
        self.total_ns as f64 / 1_000_000.0
    }
    pub fn avg_ms(&self) -> f64 {
        self.avg_ns() as f64 / 1_000_000.0
    }
    fn p99_ns(&self) -> u64 {
        pct(&self.samples, 99)
    }
}

fn pct(v: &[u64], p: u64) -> u64 {
    if v.is_empty() {
        return 0;
    }
    let mut s: Vec<u64> = v.to_vec();
    s.sort_unstable();
    let i = ((p as f64 / 100.0) * (s.len() as f64 - 1.0)).round() as usize;
    s[i.min(s.len() - 1)]
}

// --- Entity Snapshot ---
#[derive(Debug, Clone)]
pub struct EntitySnapshot {
    pub entity_type: String,
    pub x: f64,
    pub z: f64,
}

// --- Span Tree ---
#[derive(Debug, Default)]
struct SpanNode {
    name: String,
    stats: SpanStats,
    children: BTreeMap<String, SpanNode>,
}
impl SpanNode {
    fn insert(&mut self, parts: &[String], stats: &SpanStats) {
        if parts.is_empty() {
            self.stats = stats.clone();
            return;
        }
        let k = parts[0].clone();
        let c = self.children.entry(k.clone()).or_default();
        c.name = k;
        c.insert(&parts[1..], stats);
    }
    fn aggregate(&mut self) {
        for c in self.children.values_mut() {
            c.aggregate();
            self.stats.total_ns = self.stats.total_ns.saturating_add(c.stats.total_ns);
        }
        if !self.children.is_empty() {
            self.stats.count = self.children.len() as u64;
        }
    }
}

// --- Profiler ---
pub struct SpanGuard {
    start: Instant,
    span_id: u64,
    profiler: Arc<Inner>,
}
impl Drop for SpanGuard {
    fn drop(&mut self) {
        let e = self.start.elapsed();
        self.profiler.record(
            self.span_id,
            (e.as_secs() * 1_000_000_000).saturating_add(e.subsec_nanos() as u64),
        );
    }
}

struct Inner {
    enabled: AtomicBool,
    next_id: AtomicU64,
    names: Mutex<HashMap<u64, String>>,
    spans: Mutex<BTreeMap<String, SpanStats>>,
    total_ns: AtomicU64,
    session_start: Mutex<Option<Instant>>,
    entities: Mutex<Vec<EntitySnapshot>>,
}
const MAX_ENTITIES: usize = 10000;

pub struct Profiler {
    inner: Arc<Inner>,
}
impl Profiler {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                enabled: AtomicBool::new(false),
                next_id: AtomicU64::new(1),
                names: Mutex::new(HashMap::new()),
                spans: Mutex::new(BTreeMap::new()),
                total_ns: AtomicU64::new(0),
                session_start: Mutex::new(None),
                entities: Mutex::new(Vec::new()),
            }),
        }
    }
    pub fn enable(&self) {
        self.inner.enabled.store(true, Ordering::SeqCst);
        *self.inner.session_start.lock().unwrap() = Some(Instant::now());
    }
    pub fn disable(&self) {
        self.inner.enabled.store(false, Ordering::SeqCst);
    }
    pub fn is_enabled(&self) -> bool {
        self.inner.enabled.load(Ordering::SeqCst)
    }
    pub fn reset(&self) {
        self.inner.names.lock().unwrap().clear();
        self.inner.spans.lock().unwrap().clear();
        self.inner.entities.lock().unwrap().clear();
        self.inner.total_ns.store(0, Ordering::SeqCst);
        self.inner.next_id.store(1, Ordering::SeqCst);
        *self.inner.session_start.lock().unwrap() = Some(Instant::now());
    }
    pub fn span(&self, name: &str) -> SpanGuard {
        let en = self.inner.enabled.load(Ordering::Relaxed);
        let id = if en {
            let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
            self.inner
                .names
                .lock()
                .unwrap()
                .entry(id)
                .or_insert_with(|| name.to_string());
            id
        } else {
            0
        };
        SpanGuard {
            start: Instant::now(),
            span_id: id,
            profiler: self.inner.clone(),
        }
    }
    pub fn record_span(&self, name: &str, d: Duration) {
        if !self.inner.enabled.load(Ordering::Relaxed) {
            return;
        }
        let ns = (d.as_secs() * 1_000_000_000).saturating_add(d.subsec_nanos() as u64);
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        self.inner
            .names
            .lock()
            .unwrap()
            .entry(id)
            .or_insert_with(|| name.to_string());
        self.inner.record(id, ns);
    }
    pub fn record_entity(&self, entity_type: &str, x: f64, z: f64) {
        if !self.inner.enabled.load(Ordering::Relaxed) {
            return;
        }
        let mut v = self.inner.entities.lock().unwrap();
        if v.len() < MAX_ENTITIES {
            v.push(EntitySnapshot {
                entity_type: entity_type.to_string(),
                x,
                z,
            });
        }
    }
    pub fn report_html(&self) -> String {
        let spans = self.inner.spans.lock().unwrap().clone();
        let total_ns = self.inner.total_ns.load(Ordering::Relaxed);
        let ss = self
            .inner
            .session_start
            .lock()
            .unwrap()
            .map(|s| s.elapsed())
            .unwrap_or_default();
        let entities = self.inner.entities.lock().unwrap().clone();
        let total_ms = total_ns as f64 / 1_000_000.0;
        let session_s = ss.as_secs_f64();
        let mut hot: Vec<&SpanStats> = spans.values().collect();
        hot.sort_by(|a, b| b.total_ns.cmp(&a.total_ns));
        let mut root = SpanNode::default();
        for s in &hot {
            let parts: Vec<String> = s.name.split(&[':', '.']).map(|s| s.to_string()).collect();
            if !parts.is_empty() {
                root.insert(&parts, s);
            }
        }
        root.aggregate();
        gen_html(&root, &hot, total_ms, session_s, total_ns, &entities)
    }
}
impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}
impl Clone for Profiler {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Inner {
    fn record(&self, span_id: u64, ns: u64) {
        if !self.enabled.load(Ordering::Relaxed) || span_id == 0 {
            return;
        }
        let ts = self
            .session_start
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let name = self
            .names
            .lock()
            .unwrap()
            .get(&span_id)
            .cloned()
            .unwrap_or_default();
        let mut spans = self.spans.lock().unwrap();
        let e = spans.entry(name).or_default();
        e.name = self
            .names
            .lock()
            .unwrap()
            .get(&span_id)
            .cloned()
            .unwrap_or_default();
        e.record(ns, ts);
        self.total_ns.fetch_add(ns, Ordering::Relaxed);
    }
}

// --- HTML ---
fn gen_html(
    root: &SpanNode,
    flat: &[&SpanStats],
    total_ms: f64,
    session_s: f64,
    total_ns: u64,
    entities: &[EntitySnapshot],
) -> String {
    let mut h = String::with_capacity(65536);
    h.push_str(r###"<!DOCTYPE html><html lang="zh-CN"><head><meta charset="UTF-8"><title>Qexed 性能报告</title><style>
:root{--bg:#0d1117;--card:#161b22;--border:#30363d;--text:#c9d1d9;--dim:#8b949e;--accent:#58a6ff;--warn:#d29922;--hot:#f85149;--green:#3fb950}
*{margin:0;padding:0;box-sizing:border-box}body{font:13px/1.5 'Segoe UI',system-ui,sans-serif;background:var(--bg);color:var(--text);padding:20px}
h1{font-size:22px;color:var(--accent)}h2{font-size:15px;color:var(--accent);margin:20px 0 10px;border-bottom:1px solid var(--border);padding-bottom:4px}
.sub{color:var(--dim);font-size:12px;margin-bottom:16px}
.cards{display:flex;gap:12px;flex-wrap:wrap;margin-bottom:16px}
.card{background:var(--card);border:1px solid var(--border);border-radius:6px;padding:10px 14px;min-width:130px}
.card .v{font-size:22px;font-weight:600;color:var(--accent)}.card .l{font-size:11px;color:var(--dim)}
.flame{background:var(--card);border:1px solid var(--border);border-radius:6px;padding:6px;margin-bottom:16px}
.flame-row{display:flex;align-items:center;height:24px;margin:1px 0}
.flame-label{width:220px;font-size:11px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;flex-shrink:0;padding-right:8px;text-align:right;color:var(--dim)}
.flame-area{flex:1;height:20px;background:#21262d;border-radius:3px;overflow:hidden}
.flame-bar{height:100%;border-radius:3px;transition:width 0.3s;display:flex;align-items:center}.flame-bar span{font-size:9px;margin-left:4px;color:#fff}
.tree{width:100%;border-collapse:collapse;background:var(--card);border:1px solid var(--border);border-radius:6px;overflow:hidden}
.tree th{text-align:left;padding:6px 10px;background:#1c2129;font-size:11px;color:var(--dim);text-transform:uppercase;letter-spacing:.4px}
.tree td{padding:4px 10px;font-size:12px;border-top:1px solid #21262d}.tree tbody tr:hover{background:#1c2129}
.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.td-bar{width:25%}.td-bg{height:4px;background:#21262d;border-radius:2px}.td-fg{height:4px;border-radius:2px}
.td-fg.hot{background:var(--hot)}.td-fg.warn{background:var(--warn)}.td-fg.ok{background:var(--accent)}.td-fg.green{background:var(--green)}
.footer{margin-top:16px;font-size:10px;color:#484f58}
.tabs{display:flex;gap:4px;margin-bottom:12px;flex-wrap:wrap}
.tab{padding:4px 12px;background:var(--card);border:1px solid var(--border);border-radius:4px;color:var(--dim);cursor:pointer;font-size:12px;transition:all 0.2s}
.tab:hover,.tab.active{color:var(--text);border-color:var(--accent)}.tab.active{background:#1a2b3d}.tab .cnt{font-size:10px;color:var(--dim);margin-left:2px}
.summary-text{font-size:12px;color:var(--dim);margin-bottom:12px}
</style></head><body>
"###);

    write!(
        h,
        "<h1>Qexed 性能报告</h1><div class=sub>会话 {session_s:.1}s | 采样 {:.1}s | 热点 {}</div>",
        total_ms / 1000.0,
        flat.len()
    )
    .unwrap();
    write!(h, "<div class=cards>").unwrap();
    card(&mut h, &flat.len().to_string(), "热点数");
    card(&mut h, &format!("{:.1}s", total_ms / 1000.0), "总采样时间");
    card(&mut h, &format!("{:.1}s", session_s), "会话时长");
    if let Some(t) = flat.first() {
        card(
            &mut h,
            &format!("{} ({:.1}s)", t.name, t.total_ns as f64 / 1e9),
            "最大热点",
        );
    }
    card(
        &mut h,
        &flat.iter().map(|s| s.count).sum::<u64>().to_string(),
        "总调用数",
    );
    write!(h, "</div>").unwrap();

    let (ne, np, nn, nt, ni, no) = cat_counts(flat);
    write!(h, "<div class=tabs>").unwrap();
    tab(&mut h, "all", "全部", flat.len());
    tab(&mut h, "entity", "实体", ne);
    tab(&mut h, "plugin", "插件", np);
    tab(&mut h, "network", "网络", nn);
    tab(&mut h, "tick", "游戏刻", nt);
    tab(&mut h, "io", "IO", ni);
    tab(&mut h, "other", "其他", no);
    write!(h, "</div>").unwrap();

    if let Some(tick) = flat.iter().find(|s| s.name == "tick:gameplay") {
        write!(h, "<div class=summary-text>tick:gameplay 平均 {:.1}ms | p99 {:.1}ms | 约 {:.0} TPS 处理能力</div>", tick.avg_ms(), tick.p99_ns() as f64/1_000_000.0, tick.count as f64 / session_s.max(0.001)).unwrap();
    }

    // Flame
    write!(h, "<h2>火焰图</h2><div class=flame>").unwrap();
    for s in flat.iter().take(25) {
        let p = s.total_ns as f64 / total_ns.max(1) as f64 * 100.0;
        let cl = if p > 50.0 {
            "hot"
        } else if p > 20.0 {
            "warn"
        } else {
            "ok"
        };
        write!(h, "<div class=flame-row><div class=flame-label title='{}'>{}</div><div class=flame-area><div class='flame-bar {}' style='width:{:.1}%'><span>{:.1}%</span></div></div></div>", s.name, s.name, cl, p, p).unwrap();
    }
    write!(h, "</div>").unwrap();

    // Tree
    write!(h, "<h2>调用树</h2><table class=tree id=treetbl><thead><tr><th>名称</th><th>总时间</th><th>%</th><th>调用</th><th>平均</th><th>p99</th><th>最大</th><th>分布</th></tr></thead><tbody>").unwrap();
    let mut rid = 0u32;
    write_tree(&mut h, root, 0, total_ns, &mut rid);
    write!(h, "</tbody></table>").unwrap();

    // Tab JS
    h.push_str(
        r#"<script>
document.querySelectorAll('.tab').forEach(function(b){b.addEventListener('click',function(){
document.querySelectorAll('.tab').forEach(function(x){x.classList.remove('active');});
this.classList.add('active');
var cat=this.dataset.cat;
document.querySelectorAll('#treetbl tbody tr').forEach(function(r){
r.style.display=(cat==='all'||r.classList.contains('cat-'+cat))?'':'none';
});
});});
</script>"#,
    );

    // Entity section
    if !entities.is_empty() {
        let mut tc: HashMap<String, usize> = HashMap::new();
        let (mut mx, mut Mx, mut mz, mut Mz) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for e in entities {
            *tc.entry(e.entity_type.clone()).or_default() += 1;
            mx = mx.min(e.x);
            Mx = Mx.max(e.x);
            mz = mz.min(e.z);
            Mz = Mz.max(e.z);
        }
        let mut tv: Vec<_> = tc.into_iter().collect();
        tv.sort_by(|a, b| b.1.cmp(&a.1));
        write!(h, "<h2>实体分布</h2><table class=tree><thead><tr><th>类型</th><th>采样数</th></tr></thead><tbody>").unwrap();
        for (t, c) in tv.iter().take(20) {
            write!(h, "<tr><td>{t}</td><td class=num>{c}</td></tr>").unwrap();
        }
        write!(h, "</tbody></table>").unwrap();

        let mut ej = String::from("[");
        for (i, e) in entities.iter().enumerate() {
            if i > 0 {
                ej.push(',');
            }
            write!(
                ej,
                "{{\"t\":\"{}\",\"x\":{:.1},\"z\":{:.1}}}",
                e.entity_type, e.x, e.z
            )
            .unwrap();
        }
        ej.push(']');
        write!(h, "<h2>实体2D地图</h2><div style='background:var(--card);border:1px solid var(--border);border-radius:6px;padding:4px'><canvas id=emap width=800 height=400 style='width:100%;height:400px'></canvas></div>").unwrap();
        let mjs = format!(
            r##"<script>(function(){{var data={ej};var c=document.getElementById('emap');c.width=c.parentElement.clientWidth;var W=c.width,H=c.height,ctx=c.getContext('2d');ctx.fillStyle='#0d1117';ctx.fillRect(0,0,W,H);var margin=30,pxW=W-margin*2,pxH=H-margin*2;var rX=({Mx}-{mx}).max(1.0),rZ=({Mz}-{mz}).max(1.0);ctx.strokeStyle='#21262d';ctx.lineWidth=0.5;for(var i=0;i<=10;i++){{ctx.beginPath();ctx.moveTo(margin,margin+i*pxH/10);ctx.lineTo(margin+pxW,margin+i*pxH/10);ctx.stroke();}}var cols={{}};data.forEach(function(e){{var lx=margin+(e.x-{mx})/rX*pxW;var ly=margin+pxH-(e.z-{mz})/rZ*pxH;if(!cols[e.t]){{cols[e.t]='hsl('+(Object.keys(cols).length*37%360)+',70%,60%)';}}ctx.fillStyle=cols[e.t];ctx.globalAlpha=0.5;ctx.beginPath();ctx.arc(lx,ly,2.5,0,Math.PI*2);ctx.fill();}});ctx.globalAlpha=1;ctx.font='10px sans-serif';var i=0;Object.keys(cols).forEach(function(t){{ctx.fillStyle=cols[t];ctx.fillRect(8,8+i*15,10,10);ctx.fillStyle='#8b949e';ctx.fillText(t,22,8+i*15+9);i++;}});}})();</script>"##,
            mx = mx,
            Mx = Mx,
            mz = mz,
            Mz = Mz
        );
        h.push_str(&mjs);
    }

    write!(h, "<div class=footer>Qexed 性能采集器</div></body></html>").unwrap();
    h
}

fn card(h: &mut String, v: &str, l: &str) {
    write!(
        h,
        "<div class=card><div class=v>{v}</div><div class=l>{l}</div></div>"
    )
    .unwrap();
}
fn tab(h: &mut String, cat: &str, l: &str, c: usize) {
    let a = if cat == "all" { " active" } else { "" };
    write!(
        h,
        "<div class='tab{a}' data-cat='{cat}'>{l}<span class=cnt>{c}</span></div>"
    )
    .unwrap();
}

fn cat(name: &str) -> &'static str {
    if name.starts_with("entity:") {
        "entity"
    } else if name.starts_with("plugin:")
        || name.contains("block_drops")
        || name.contains("block_step")
    {
        "plugin"
    } else if name.starts_with("net:") || name.starts_with("world:") {
        "network"
    } else if name.starts_with("tick:") || name.starts_with("move:") || name.starts_with("pkt:") {
        "tick"
    } else if name.starts_with("io:") || name.starts_with("event:") {
        "io"
    } else {
        "other"
    }
}
fn cat_counts(f: &[&SpanStats]) -> (usize, usize, usize, usize, usize, usize) {
    let (mut e, mut p, mut n, mut t, mut i, mut o) = (0, 0, 0, 0, 0, 0);
    for s in f {
        match cat(&s.name) {
            "entity" => {
                e += 1;
            }
            "plugin" => {
                p += 1;
            }
            "network" => {
                n += 1;
            }
            "tick" => {
                t += 1;
            }
            "io" => {
                i += 1;
            }
            _ => {
                o += 1;
            }
        }
    }
    (e, p, n, t, i, o)
}

fn write_tree(h: &mut String, node: &SpanNode, depth: usize, total_ns: u64, rid: &mut u32) {
    let pct = node.stats.total_ns as f64 / total_ns.max(1) as f64 * 100.0;
    let cls = if pct > 40.0 {
        "hot"
    } else if pct > 15.0 {
        "warn"
    } else if pct > 3.0 {
        "ok"
    } else {
        "green"
    };
    let category = cat(&node.stats.name);
    *rid += 1;
    let indent = if depth == 0 {
        String::new()
    } else {
        let mut s = String::new();
        for _ in 0..depth.saturating_sub(1) {
            s.push_str("<span style=color:#30363d>| </span>");
        }
        s.push_str("<span style=color:#30363d>|-</span>");
        s
    };
    let display = if depth == 0 {
        &node.name
    } else {
        node.name.rsplit(&[':', '.']).next().unwrap_or(&node.name)
    };
    write!(h, "<tr class='cat-{category} tab-row all show'>").unwrap();
    write!(h, "<td>{indent} {display}</td>").unwrap();
    write!(
        h,
        "<td class=num>{:.1}ms</td>",
        node.stats.total_ns as f64 / 1_000_000.0
    )
    .unwrap();
    write!(h, "<td class=num>{:.1}%</td>", pct).unwrap();
    let cn = if depth == 0 && !node.children.is_empty() {
        "-".into()
    } else {
        node.stats.count.to_string()
    };
    write!(h, "<td class=num>{cn}</td>").unwrap();
    write!(h, "<td class=num>{:.2}ms</td>", node.stats.avg_ms()).unwrap();
    write!(
        h,
        "<td class=num>{:.1}ms</td>",
        node.stats.p99_ns() as f64 / 1_000_000.0
    )
    .unwrap();
    write!(
        h,
        "<td class=num>{:.1}ms</td>",
        node.stats.max_ns as f64 / 1_000_000.0
    )
    .unwrap();
    write!(h, "<td class=td-bar><div class=td-bg><div class='td-fg {cls}' style='width:{:.1}%'></div></div></td>", pct.min(100.0)).unwrap();
    write!(h, "</tr>").unwrap();
    let mut kids: Vec<&SpanNode> = node.children.values().collect();
    kids.sort_by(|a, b| b.stats.total_ns.cmp(&a.stats.total_ns));
    for k in kids {
        if (k.stats.total_ns as f64 / total_ns.max(1) as f64) < 0.001 {
            continue;
        }
        write_tree(h, k, depth + 1, total_ns, rid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled() {
        let p = Profiler::new();
        {
            let _ = p.span("t");
        }
        let h = p.report_html();
        assert!(h.contains("热点数") && h.contains(">0<"));
    }
    #[test]
    fn records() {
        let p = Profiler::new();
        p.enable();
        {
            let _ = p.span("physics");
            std::thread::sleep(Duration::from_millis(2));
        }
        p.disable();
        assert!(p.report_html().contains("physics"));
    }
    #[test]
    fn reset() {
        let p = Profiler::new();
        p.enable();
        {
            let _ = p.span("old");
            std::thread::sleep(Duration::from_millis(1));
        }
        p.reset();
        {
            let _ = p.span("new");
            std::thread::sleep(Duration::from_millis(1));
        }
        p.disable();
        let h = p.report_html();
        assert!(!h.contains("old"));
        assert!(h.contains("new"));
    }
    #[test]
    fn merge() {
        let p = Profiler::new();
        p.enable();
        {
            let _ = p.span("r");
        }
        {
            let _ = p.span("r");
        }
        p.disable();
        assert!(p.report_html().contains("热点数") && p.report_html().contains(">1<"));
    }
    #[test]
    fn html() {
        let p = Profiler::new();
        p.enable();
        {
            let _ = p.span("tick:gameplay:redstone");
            std::thread::sleep(Duration::from_millis(5));
        }
        {
            let _ = p.span("tick:gameplay");
            std::thread::sleep(Duration::from_millis(10));
        }
        p.disable();
        let h = p.report_html();
        assert!(h.contains("tick:gameplay"));
        assert!(h.contains("redstone"));
        assert!(h.contains("调用树"));
        assert!(h.contains("火焰图"));
    }
    #[test]
    fn tree() {
        let p = Profiler::new();
        p.enable();
        {
            let _ = p.span("a:b:c");
            std::thread::sleep(Duration::from_millis(1));
        }
        {
            let _ = p.span("a:b:d");
            std::thread::sleep(Duration::from_millis(1));
        }
        p.disable();
        let h = p.report_html();
        assert!(h.contains("c"));
        assert!(h.contains("d"));
    }
    #[test]
    fn concurrent() {
        let p = Profiler::new();
        p.enable();
        std::thread::scope(|s| {
            for ti in 0..4 {
                let p = &p;
                s.spawn(move || {
                    for _ in 0..100 {
                        let _ = p.span(&format!("thread_{ti}"));
                    }
                });
            }
        });
        p.disable();
        let h = p.report_html();
        for ti in 0..4 {
            assert!(h.contains(&format!("thread_{ti}")));
        }
    }
    #[test]
    fn entities() {
        let p = Profiler::new();
        p.enable();
        p.record_entity("minecraft:zombie", 10.0, 20.0);
        p.record_entity("minecraft:creeper", -5.0, 15.0);
        p.record_entity("minecraft:zombie", 12.0, 22.0);
        p.disable();
        let h = p.report_html();
        assert!(h.contains("minecraft:zombie"));
        assert!(h.contains("minecraft:creeper"));
        assert!(h.contains("实体2D地图"));
    }
}

/// 全局 profiler 句柄（v4 console 用法的 v6 等价；未初始化返回 None）。
static GLOBAL: std::sync::OnceLock<std::sync::Arc<Profiler>> = std::sync::OnceLock::new();

pub fn init_global(profiler: std::sync::Arc<Profiler>) {
    let _ = GLOBAL.set(profiler);
}

pub fn get() -> Option<std::sync::Arc<Profiler>> {
    GLOBAL.get().cloned()
}
