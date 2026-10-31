//! Removal review ownership shared with the Qt bridge. A cancelled async preview
//! cannot restore a pending plan, and confirmation consumes the plan exactly once.
use crate::providers::PreparedRemoval;
#[derive(Default)]
pub struct PendingRemoval {
    generation: u64,
    pending: Option<Box<dyn PreparedRemoval>>,
}
impl PendingRemoval {
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn accept(
        &mut self,
        generation: u64,
        selected: &str,
        plan: Box<dyn PreparedRemoval>,
    ) -> bool {
        if generation != self.generation || plan.review().requested.key() != selected {
            return false;
        }
        self.pending = Some(plan);
        true
    }
    pub fn take(&mut self, selected: &str) -> Result<Box<dyn PreparedRemoval>, String> {
        let plan = self.pending.take().ok_or("No removal plan is ready")?;
        if plan.review().requested.key() != selected {
            return Err("Selection changed. Review a new removal plan.".into());
        }
        Ok(plan)
    }
    pub fn review(&self) -> Option<&crate::model::RemovalReview> {
        self.pending.as_ref().map(|p| p.review())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{PackageId, RemovalReview},
        providers,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Plan {
        review: RemovalReview,
        dropped: Arc<AtomicUsize>,
    }
    impl Drop for Plan {
        fn drop(&mut self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl PreparedRemoval for Plan {
        fn review(&self) -> &RemovalReview {
            &self.review
        }
        fn execute(self: Box<Self>) -> providers::Result<String> {
            panic!("review tests must never execute")
        }
    }
    fn plan(dropped: &Arc<AtomicUsize>) -> Box<dyn PreparedRemoval> {
        Box::new(Plan {
            review: RemovalReview {
                executor_label: "test".into(),
                requested: PackageId {
                    provider: "rpm".into(),
                    scope: "system".into(),
                    native: "example-1-1.x86_64".into(),
                },
                changes: vec![],
                notes: vec![],
            },
            dropped: dropped.clone(),
        })
    }
    #[test]
    fn cancelled_async_preview_is_dropped_instead_of_reopened() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut slot = PendingRemoval::default();
        let token = slot.generation();
        let p = plan(&count);
        let key = p.review().requested.key();
        slot.invalidate();
        assert!(!slot.accept(token, &key, p));
        assert!(slot.review().is_none());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn selection_changes_reject_preview_and_confirmation() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut slot = PendingRemoval::default();
        assert!(!slot.accept(slot.generation(), "another-package", plan(&count)));
        let p = plan(&count);
        let key = p.review().requested.key();
        assert!(slot.accept(slot.generation(), &key, p));
        assert!(slot.take("another-package").is_err());
        assert!(slot.take(&key).is_err());
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn reviewed_plan_can_be_taken_only_once_and_discard_cleans_it() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut slot = PendingRemoval::default();
        let p = plan(&count);
        let key = p.review().requested.key();
        assert!(slot.accept(slot.generation(), &key, p));
        drop(slot.take(&key).unwrap());
        assert!(slot.take(&key).is_err());
        assert!(slot.accept(slot.generation(), &key, plan(&count)));
        slot.invalidate();
        assert!(slot.review().is_none());
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}
