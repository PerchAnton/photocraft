//! #6423: compound operands form a graph, not necessarily a tree.
#![cfg(feature = "synth")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use photocraft_affinity::model::{Document, Kind};
use photocraft_affinity::synth::{self, F, Method, tag};
use photocraft_affinity::{Limits, read};
use std::time::{Duration, Instant};

fn compound(id: u32, children: Vec<F>) -> F {
    F::Def(id, vec![tag(b"Comp")], vec![(tag(b"Chld"), F::Shared(children))])
}

fn rectangle() -> F {
    F::Def(100, vec![tag(b"ShpN")], vec![(tag(b"ShpB"), F::F64s(vec![0.0, 0.0, 10.0, 20.0])), (tag(b"Shpe"), F::Def(101, vec![tag(b"ShNR")], vec![]))])
}

fn document(definitions: Vec<F>, roots: Vec<F>) -> Document {
    let data = synth::stream(&[
        (tag(b"Defs"), F::Shared(definitions)),
        (
            tag(b"DocR"),
            F::Def(
                1,
                vec![tag(b"DocN")],
                vec![(
                    tag(b"Chld"),
                    F::Shared(vec![F::Def(
                        2,
                        vec![tag(b"Sprd")],
                        vec![(tag(b"SprB"), F::F64s(vec![0.0, 0.0, 400.0, 300.0])), (tag(b"Chld"), F::Shared(roots))],
                    )]),
                )],
            ),
        ),
    ]);
    read(&synth::container(&[("doc.dat", &data, Method::Zlib)], None), Limits::default()).unwrap()
}

// A stack overflow aborts rather than unwinds. Keep that failure (and an exponential hang)
// inside a child process, so the regression fails without killing the entire test runner.
fn isolated(name: &str, check: impl FnOnce()) {
    if std::env::var("PHOTOCRAFT_COMPOUND_CASE").as_deref() == Ok(name) {
        check();
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("PHOTOCRAFT_COMPOUND_CASE", name)
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "compound import failed: {status}");
            break;
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("compound import exceeded ten seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn unsupported(doc: &Document, warning: &str) {
    assert!(matches!(doc.spreads[0].nodes[0].kind, Kind::Unsupported));
    assert!(doc.warnings.iter().any(|w| w.contains(warning)), "{:?}", doc.warnings);
}

#[test]
fn self_referencing_compound_is_reported_without_aborting() {
    isolated("self_referencing_compound_is_reported_without_aborting", || {
        let doc = document(vec![], vec![compound(10, vec![F::Ref(10)]), rectangle()]);
        unsupported(&doc, "cyclic compound");
        assert!(matches!(doc.spreads[0].nodes[1].kind, Kind::Shape { .. }), "a cycle must not poison unrelated shapes");
    });
}

#[test]
fn indirectly_cyclic_compound_is_not_imported_as_a_partial_shape() {
    isolated("indirectly_cyclic_compound_is_not_imported_as_a_partial_shape", || {
        let doc = document(vec![rectangle()], vec![compound(10, vec![F::Ref(100), compound(11, vec![F::Ref(10)])])]);
        unsupported(&doc, "cyclic compound");
    });
}

fn nested(levels: u32, repeated: bool) -> Document {
    // Flat definitions bypass the stream parser's nesting cap: only references form the depth.
    let mut definitions = vec![rectangle()];
    let mut previous = 100;
    for id in 200..200 + levels {
        let children = if repeated { vec![F::Ref(previous), F::Ref(previous)] } else { vec![F::Ref(previous)] };
        definitions.push(compound(id, children));
        previous = id;
    }
    document(definitions, vec![F::Ref(previous)])
}

#[test]
fn deep_compound_graph_is_bounded_on_a_small_stack() {
    isolated("deep_compound_graph_is_bounded_on_a_small_stack", || {
        std::thread::Builder::new().stack_size(512 * 1024).spawn(|| unsupported(&nested(1000, false), "compound depth")).unwrap().join().unwrap();
    });
}

#[test]
fn shared_operands_cannot_expand_exponentially() {
    isolated("shared_operands_cannot_expand_exponentially", || unsupported(&nested(40, true), "compound geometry budget"));
}

#[test]
fn shared_empty_operands_also_have_a_work_limit() {
    isolated("shared_empty_operands_also_have_a_work_limit", || {
        let mut definitions = vec![compound(100, vec![])];
        let mut previous = 100;
        for id in 200..240 {
            definitions.push(compound(id, vec![F::Ref(previous), F::Ref(previous)]));
            previous = id;
        }
        unsupported(&document(definitions, vec![F::Ref(previous)]), "compound geometry budget");
    });
}

#[test]
fn geometry_budget_is_shared_across_layer_roots() {
    isolated("geometry_budget_is_shared_across_layer_roots", || {
        let mut definitions = vec![rectangle()];
        let mut previous = 100;
        for id in 200..214 {
            definitions.push(compound(id, vec![F::Ref(previous), F::Ref(previous)]));
            previous = id;
        }
        let mut roots = vec![F::Ref(previous); 12];
        roots.push(F::Ref(100));
        let doc = document(definitions, roots);
        let nodes = &doc.spreads[0].nodes;
        assert!(matches!(nodes[0].kind, Kind::Shape { .. }), "a modest shared graph still imports");
        assert!(matches!(nodes.last().unwrap().kind, Kind::Unsupported), "later roots cannot reset the exhausted budget");
        assert!(doc.warnings.iter().any(|w| w.contains("compound geometry budget")));
    });
}

#[test]
fn ordinary_shared_operands_keep_both_transformed_copies() {
    let mut translated = compound(201, vec![F::Ref(100)]);
    let F::Def(_, _, fields) = &mut translated else { unreachable!() };
    fields.push((tag(b"Xfrm"), F::F64s(vec![1.0, 0.0, 30.0, 0.0, 1.0, 40.0])));
    let doc = document(vec![rectangle(), compound(200, vec![F::Ref(100)]), translated], vec![compound(202, vec![F::Ref(200), F::Ref(201)])]);
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    let Kind::Shape { path, .. } = &doc.spreads[0].nodes[0].kind else { panic!("expected compound outline") };
    assert_eq!(path.subpaths.len(), 2);
    for (a, b) in path.subpaths[0].segments.iter().flatten().zip(path.subpaths[1].segments.iter().flatten()) {
        assert_eq!(b.x, a.x + 30.0);
        assert_eq!(b.y, a.y + 40.0);
    }
}
