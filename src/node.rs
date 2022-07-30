use anymap::AnyMap;
use dioxus::prelude::*;
use dioxus_native_core::node_ref::{AttributeMask, NodeMask, NodeView};
use dioxus_native_core::real_dom::RealDom;
use dioxus_native_core::state::{ChildDepState, NodeDepState, State};
use dioxus_native_core_macro::{sorted_str_slice, State};
use skia_safe::*;
use std::sync::Mutex;
use std::sync::{mpsc, Arc};

use crate::run;

#[derive(Debug, Clone, State, Default)]
pub struct NodeState {
    #[child_dep_state(size, f32)]
    pub size: Size,
    #[node_dep_state()]
    pub style: Style,
}

#[derive(Default, Copy, Clone, Debug)]
pub struct Size(pub f32, pub f32);

impl ChildDepState for Size {
    // Size accepts a font size context
    type Ctx = f32;
    // Size depends on the Size part of each child
    type DepState = Self;
    // Size only cares about the width, height, and text parts of the current node
    const NODE_MASK: NodeMask =
        NodeMask::new_with_attrs(AttributeMask::Static(&sorted_str_slice!([
            "width", "height"
        ])))
        .with_text();
    fn reduce<'a>(
        &mut self,
        node: NodeView,
        mut children: impl Iterator<Item = &'a Self::DepState>,
        _ctx: &Self::Ctx,
    ) -> bool
    where
        Self::DepState: 'a,
    {
        let mut width;
        let mut height;

        width = children
            .by_ref()
            .map(|item| item.0)
            .reduce(|accum, item| if accum >= item { accum } else { item })
            .unwrap_or(0.0);

        height = children
            .map(|item| item.1)
            .reduce(|accum, item| if accum >= item { accum } else { item })
            .unwrap_or(0.0);
        // if the node contains a width or height attribute it overrides the other size
        for a in node.attributes() {
            match a.name {
                "width" => width = a.value.to_string().parse().unwrap(),
                "height" => height = a.value.to_string().parse().unwrap(),
                // because Size only depends on the width and height, no other attributes will be passed to the member
                _ => panic!(),
            }
        }
        // to determine what other parts of the dom need to be updated we return a boolean that marks if this member changed
        let changed = (width != self.0) || (height != self.1);
        *self = Self(width, height);
        changed
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub struct Style {
    pub background: Color,
}

impl NodeDepState<()> for Style {
    type Ctx = ();

    const NODE_MASK: NodeMask =
        NodeMask::new_with_attrs(AttributeMask::Static(&sorted_str_slice!(["background"])))
            .with_text();
    fn reduce<'a>(&mut self, node: NodeView, _sibling: (), _ctx: &Self::Ctx) -> bool {
        let mut background = Color::TRANSPARENT;

        // if the node contains a width or height attribute it overrides the other size
        for attr in node.attributes() {
            match attr.name {
                "background" => {
                    let new_back = color_str(&attr.value.to_string());
                    if let Some(new_back) = new_back {
                        background = new_back;
                    }
                }
                _ => panic!(),
            }
        }
        // to determine what other parts of the dom need to be updated we return a boolean that marks if this member changed
        let changed = background != self.background;
        *self = Self { background };
        changed
    }
}

fn color_str(color: &str) -> Option<Color> {
    match color {
        "red" => Some(Color::RED),
        "green" => Some(Color::GREEN),
        "blue" => Some(Color::BLUE),
        _ => None,
    }
}

pub fn launch(app: Component<()>) {
    let rdom = Arc::new(Mutex::new(RealDom::<NodeState>::new()));
    let (trig_render, rev_render) = mpsc::channel::<()>();

    {
        let rdom = rdom.clone();
        std::thread::spawn(move || {
            let mut dom = VirtualDom::new(app);

            let muts = dom.rebuild();
            let to_update = rdom.lock().unwrap().apply_mutations(vec![muts]);
            let mut ctx = AnyMap::new();
            ctx.insert(0.0f32);
            rdom.lock().unwrap().update_state(&dom, to_update, ctx);
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    loop {
                        dom.wait_for_work().await;
                        let mutations = dom.work_with_deadline(|| false);
                        let to_update = rdom.lock().unwrap().apply_mutations(mutations);
                        let ctx = AnyMap::new();
                        if !to_update.is_empty() {
                            trig_render.send(()).unwrap();
                        }
                        rdom.lock().unwrap().update_state(&dom, to_update, ctx);
                    }
                });
        });
    }

    run::run(rdom.clone(), rev_render);
}