use crate::export_item::ExportItem;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// 收集 [path 段..., name] 作为相对路由段。
/// 与 qexed_config::build_safe_path 同规则：前导 '/' 归一化为相对路径、
/// 拒绝 `..` / `.` / 绝对段。
pub fn route_segments(item: &ExportItem) -> anyhow::Result<Vec<OsString>> {
    let mut segments = Vec::new();
    let sub_trimmed = item.path.trim_start_matches('/');
    for comp in Path::new(sub_trimmed).components() {
        match comp {
            Component::Normal(seg) => segments.push(seg.to_os_string()),
            Component::ParentDir | Component::CurDir => {
                anyhow::bail!(
                    "非法路径段：{comp:?}（path={:?}, name={:?}）",
                    item.path,
                    item.name
                );
            }
            Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!(
                    "不支持绝对路径或盘符前缀：{comp:?}（path={:?}, name={:?}）",
                    item.path,
                    item.name
                );
            }
        }
    }
    if !item.name.is_empty() {
        segments.push(item.name.into());
    }
    Ok(segments)
}

/// 把 route_segments 的结果拼成 URL 路径（统一用 '/'，不随平台变）。
pub fn url_path(segs: &[OsString]) -> String {
    segs.iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[derive(Debug, Default)]
struct RouteNode {
    /// 该路由段本身对应的 schema 条目索引。空 = 只是中间分组。
    items: Vec<usize>,
    children: BTreeMap<String, RouteNode>,
}

impl RouteNode {
    fn insert(&mut self, segments: &[OsString], idx: usize) {
        match segments.split_first() {
            None => self.items.push(idx),
            Some((head, rest)) => {
                let key = head.to_string_lossy().into_owned();
                self.children.entry(key).or_default().insert(rest, idx);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    /// 具体配置页：渲染 schema。
    Config,
    /// 中间层路由列表：列出子路由链接。
    Index,
}

impl PageKind {
    pub fn label(self) -> &'static str {
        match self {
            PageKind::Config => "config",
            PageKind::Index => "index",
        }
    }
}

#[derive(Debug)]
pub struct RoutePage {
    /// 相对 out/<lang> 的目录，如 "a/b/c/x"。
    pub rel_dir: PathBuf,
    /// 该路由绑定的 config 条目索引。
    pub items: Vec<usize>,
    /// 子路由段名（Index 页用来列链接）。
    pub child_segments: Vec<String>,
}

impl RoutePage {
    pub fn kind(&self) -> PageKind {
        if self.items.is_empty() {
            PageKind::Index
        } else {
            PageKind::Config
        }
    }
}

/// 从 items 构建路由树并展开为页面列表。
pub fn collect_pages(items: &[ExportItem]) -> anyhow::Result<Vec<RoutePage>> {
    let mut root = RouteNode::default();
    for (idx, item) in items.iter().enumerate() {
        let segs = route_segments(item)?;
        if segs.is_empty() {
            anyhow::bail!(
                "空路由（path={:?}, name={:?}）：至少要有一段",
                item.path,
                item.name
            );
        }
        root.insert(&segs, idx);
    }

    let mut pages = Vec::new();
    walk(&root, PathBuf::new(), &mut pages);

    // 冲突检查：同一路由不能对应多个 schema（Index 节点天然无 items）。
    for page in &pages {
        if page.items.len() > 1 {
            anyhow::bail!(
                "路由 {:?} 被多个 schema 声明（索引 {:?}）",
                page.rel_dir,
                page.items
            );
        }
    }
    Ok(pages)
}

fn walk(node: &RouteNode, rel: PathBuf, out: &mut Vec<RoutePage>) {
    for (seg, child) in &node.children {
        let mut child_rel = rel.clone();
        child_rel.push(seg);
        out.push(RoutePage {
            rel_dir: child_rel.clone(),
            items: child.items.clone(),
            child_segments: child.children.keys().cloned().collect(),
        });
        walk(child, child_rel, out);
    }
}
