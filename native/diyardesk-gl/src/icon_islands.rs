use crate::state::Rect;

/// Gap-based connected-component clustering for desktop icon bounding boxes.
///
/// Merges nearby icon rectangles into cohesive bounding-box "islands" whenever
/// two boxes lie within `gap_px` horizontally and vertically (including
/// transitive chains via union-find). Returns components ordered deterministically
/// by each cluster's earliest input index.
#[allow(dead_code)]
pub fn cluster_icons(icons: &[Rect], gap_px: f32) -> Vec<Rect> {
    cluster_icons_with_names(icons, &[], gap_px).0
}

/// Clusters desktop icon rectangles and produces aligned cluster labels.
pub fn cluster_icons_with_names(
    icons: &[Rect],
    names: &[String],
    gap_px: f32,
) -> (Vec<Rect>, Vec<String>) {
    if icons.is_empty() {
        return (Vec::new(), Vec::new());
    }
    if icons.len() == 1 || !gap_px.is_finite() || gap_px <= 0.0 {
        let mut out_names = Vec::with_capacity(icons.len());
        for i in 0..icons.len() {
            out_names.push(names.get(i).cloned().unwrap_or_default());
        }
        return (icons.to_vec(), out_names);
    }

    let n = icons.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            let gp = parent[parent[x]];
            parent[x] = gp;
            x = gp;
        }
        x
    }

    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            if ra < rb {
                parent[rb] = ra;
            } else {
                parent[ra] = rb;
            }
        }
    }

    for i in 0..n {
        let a = icons[i];
        if a.w <= 0.0 || a.h <= 0.0 {
            continue;
        }
        let a_r = a.x + a.w;
        let a_b = a.y + a.h;
        for j in (i + 1)..n {
            let b = icons[j];
            if b.w <= 0.0 || b.h <= 0.0 {
                continue;
            }
            let b_r = b.x + b.w;
            let b_b = b.y + b.h;
            let dx = (a.x - b_r).max(b.x - a_r).max(0.0);
            let dy = (a.y - b_b).max(b.y - a_b).max(0.0);
            if dx <= gap_px && dy <= gap_px {
                union(&mut parent, i, j);
            }
        }
    }

    // Group members deterministically by smallest index root
    let mut root_to_cluster: Vec<(usize, Rect, usize, String)> = Vec::new();
    let mut root_pos: Vec<Option<usize>> = vec![None; n];

    for i in 0..n {
        let r = icons[i];
        if r.w <= 0.0 || r.h <= 0.0 {
            continue;
        }
        let root = find(&mut parent, i);
        let nm = names.get(i).map(|s| s.trim()).unwrap_or("");
        if let Some(pos) = root_pos[root] {
            let entry = &mut root_to_cluster[pos];
            let min_x = entry.1.x.min(r.x);
            let min_y = entry.1.y.min(r.y);
            let max_r = (entry.1.x + entry.1.w).max(r.x + r.w);
            let max_b = (entry.1.y + entry.1.h).max(r.y + r.h);
            entry.1 = Rect {
                x: min_x,
                y: min_y,
                w: (max_r - min_x).max(1.0),
                h: (max_b - min_y).max(1.0),
            };
            entry.2 += 1;
            if entry.3.is_empty() && !nm.is_empty() {
                entry.3 = nm.to_string();
            }
        } else {
            let idx = root_to_cluster.len();
            root_pos[root] = Some(idx);
            root_to_cluster.push((root, r, 1, nm.to_string()));
        }
    }

    let mut out_rects = Vec::with_capacity(root_to_cluster.len());
    let mut out_names = Vec::with_capacity(root_to_cluster.len());
    for (_, rect, count, first_name) in root_to_cluster {
        out_rects.push(rect);
        if count <= 1 {
            out_names.push(first_name);
        } else if !first_name.is_empty() {
            out_names.push(format!("{first_name} (+{})", count - 1));
        } else {
            out_names.push(format!("Island ({count})"));
        }
    }
    (out_rects, out_names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn empty_and_single_icon_passthrough() {
        assert!(cluster_icons(&[], 36.0).is_empty());
        let single = vec![rect(10.0, 20.0, 64.0, 80.0)];
        let out = cluster_icons(&single, 36.0);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].x, out[0].y, out[0].w, out[0].h), (10.0, 20.0, 64.0, 80.0));
    }

    #[test]
    fn merges_nearby_icons_into_two_distinct_islands() {
        let icons = vec![
            rect(20.0, 20.0, 60.0, 70.0),
            rect(95.0, 22.0, 60.0, 70.0),   // dx = 15 <= 36 -> Island 1
            rect(24.0, 110.0, 60.0, 70.0),  // dy = 20 <= 36 -> Island 1
            rect(800.0, 500.0, 60.0, 70.0), // far away -> Island 2
            rect(880.0, 500.0, 60.0, 70.0), // dx = 20 <= 36 -> Island 2
        ];
        let names = vec![
            "Docs".to_string(),
            "Notes".to_string(),
            "Code".to_string(),
            "Steam".to_string(),
            "Obsidian".to_string(),
        ];
        let (out_r, out_n) = cluster_icons_with_names(&icons, &names, 36.0);
        assert_eq!(out_r.len(), 2);
        assert_eq!(out_n.len(), 2);
        assert_eq!((out_r[0].x, out_r[0].y, out_r[0].w, out_r[0].h), (20.0, 20.0, 135.0, 160.0));
        assert_eq!((out_r[1].x, out_r[1].y, out_r[1].w, out_r[1].h), (800.0, 500.0, 140.0, 70.0));
        assert_eq!(out_n[0], "Docs (+2)");
        assert_eq!(out_n[1], "Steam (+1)");
    }

    #[test]
    fn transitive_chain_merges_and_non_positive_gap_is_safe() {
        let chain = vec![
            rect(0.0, 0.0, 40.0, 40.0),
            rect(60.0, 0.0, 40.0, 40.0),
            rect(120.0, 0.0, 40.0, 40.0),
        ];
        let merged = cluster_icons(&chain, 24.0);
        assert_eq!(merged.len(), 1);
        assert_eq!((merged[0].x, merged[0].y, merged[0].w, merged[0].h), (0.0, 0.0, 160.0, 40.0));

        assert_eq!(cluster_icons(&chain, 0.0).len(), 3);
        assert_eq!(cluster_icons(&chain, f32::NAN).len(), 3);
    }
}
