use egg::{Analysis, DidMerge};
use eggmock::NetworkLanguage;

#[derive(Default)]
pub struct LimeAnalysis;

#[derive(Debug)]
pub struct LimeAnalysisData {
    // rough lower bound for the size of a term from the eclass
    pub min_size: usize,
}

impl<L: NetworkLanguage> Analysis<L> for LimeAnalysis {
    type Data = LimeAnalysisData;

    fn make(egraph: &mut egg::EGraph<L, Self>, enode: &L) -> Self::Data {
        let delta = if enode.is_not() { 0 } else { 1 };
        LimeAnalysisData {
            min_size: enode
                .children()
                .iter()
                .map(|id| egraph[egraph.find(*id)].data.min_size)
                .max()
                .unwrap_or(0)
                + delta,
        }
    }

    fn merge(&mut self, a: &mut Self::Data, b: Self::Data) -> egg::DidMerge {
        if a.min_size == b.min_size {
            DidMerge(false, false)
        } else if a.min_size < b.min_size {
            DidMerge(false, true)
        } else {
            a.min_size = b.min_size;
            DidMerge(true, false)
        }
    }
}
