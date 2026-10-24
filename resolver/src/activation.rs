//! Pure activation of a fully locked union graph. No filesystem or network I/O.
use diagnostic::Error;
use lockfile::LockedPackage;
use source::graph::{Context, Edge, Root, Selection};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Active {
    pub packages: Vec<LockedPackage>,
    pub roots: Vec<String>,
    pub features: BTreeMap<String, BTreeSet<String>>,
}
#[derive(Default, Clone, Eq, PartialEq)]
struct Request {
    features: BTreeSet<String>,
    defaults: bool,
}
fn identity(package: &LockedPackage) -> String {
    package.id.clone().unwrap_or_else(|| {
        lockfile::package_identity(
            package.package_name(),
            &package.version,
            &package.source,
            package.subdir.as_deref(),
        )
    })
}
fn edges(package: &LockedPackage, packages: &BTreeMap<&str, &LockedPackage>) -> Vec<Edge> {
    if !package.edges.is_empty() {
        return package.edges.clone();
    }
    package
        .dependencies
        .iter()
        .map(|key| Edge::unconditional(key.clone(), packages[key.as_str()].import_name().into()))
        .collect()
}
fn selected(edges: &[Edge], enabled: &BTreeSet<String>, target: &Context) -> Vec<Edge> {
    edges
        .iter()
        .filter(|edge| {
            edge.target.as_ref().is_none_or(|p| p.matches(target))
                && (!edge.optional || enabled.contains(&edge.alias))
        })
        .cloned()
        .collect()
}

pub(crate) fn select(
    root: &Root,
    union: &[LockedPackage],
    selection: &Selection,
) -> Result<Active, Error> {
    let target = selection.target.clone().unwrap_or_else(Context::host);
    let packages: BTreeMap<_, _> = union.iter().map(|p| (p.name.as_str(), p)).collect();
    let root_features = source::graph::expand(
        &root.features,
        &root.edges,
        &selection.features,
        !selection.no_default_features,
    )
    .map_err(Error::resolution)?;
    let root_edges = selected(&root.edges, &root_features.enabled, &target);
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut requests: BTreeMap<String, Request> = BTreeMap::new();
    loop {
        let previous = (reached.clone(), requests.clone());
        let mut parents = vec![(root_edges.clone(), root_features.forwarded.clone())];
        for key in &reached {
            let package = packages
                .get(key.as_str())
                .ok_or_else(|| Error::resolution("incomplete union graph"))?;
            let request = requests
                .get(&identity(package))
                .cloned()
                .unwrap_or_default();
            let all_edges = edges(package, &packages);
            let expanded = source::graph::expand(
                &package.features,
                &all_edges,
                &request.features,
                request.defaults,
            )
            .map_err(|e| Error::resolution(e).with_field("package", package.package_name()))?;
            parents.push((
                selected(&all_edges, &expanded.enabled, &target),
                expanded.forwarded,
            ));
        }
        for (edges, forwarded) in parents {
            for edge in edges {
                let child = packages
                    .get(edge.key.as_str())
                    .ok_or_else(|| Error::resolution("activation edge has no locked source"))?;
                reached.insert(edge.key);
                let request = requests.entry(identity(child)).or_default();
                request.defaults |= edge.default_features;
                request.features.extend(edge.features);
                if let Some(features) = forwarded.get(&edge.alias) {
                    request.features.extend(features.iter().cloned());
                }
            }
        }
        if previous == (reached.clone(), requests.clone()) {
            break;
        }
    }
    let mut aliases: BTreeMap<&str, String> = BTreeMap::new();
    let mut active = Vec::new();
    let mut features = BTreeMap::new();
    for key in reached {
        let package = packages[key.as_str()];
        let alias = package.import_name();
        if alias == root.name {
            return Err(Error::resolution(format!(
                "dependency alias `{alias}` conflicts with the root package name"
            )));
        }
        if let Some(previous) = aliases.insert(alias, identity(package)) {
            if previous != identity(package) {
                return Err(Error::resolution(format!("import alias `{alias}` refers to multiple active package sources; Wave currently exposes a global import-name map\nhelp: choose distinct aliases; scoped compiler dependency mappings are required for conflicting transitive imports")));
            }
        }
        let request = requests
            .get(&identity(package))
            .cloned()
            .unwrap_or_default();
        let all_edges = edges(package, &packages);
        let expanded = source::graph::expand(
            &package.features,
            &all_edges,
            &request.features,
            request.defaults,
        )
        .map_err(Error::resolution)?;
        features.insert(identity(package), expanded.features);
        let mut package = package.clone();
        package.dependencies = selected(&all_edges, &expanded.enabled, &target)
            .into_iter()
            .map(|e| e.key)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        active.push(package);
    }
    features.insert(format!("root:{}", root.manifest), root_features.features);
    Ok(Active {
        packages: active,
        roots: root_edges
            .into_iter()
            .map(|e| e.key)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        features,
    })
}
