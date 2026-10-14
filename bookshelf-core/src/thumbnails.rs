//! The search list's cover thumbnails, kept in memory for the session, and
//! the turns that keep their downloads to a few at a time. Used by
//! `OpenLibrary::search_thumbnail`.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// How many thumbnails download at once; the rest wait their turn.
const MAX_FETCHES: usize = 3;
/// The most thumbnail bytes kept. A medium cover is about 20 KB, so this
/// holds a good many searches' worth.
const MAX_KEPT_BYTES: usize = 4 * 1024 * 1024;
/// The most thumbnails kept, counting "no cover" answers.
const MAX_KEPT: usize = 200;

#[derive(Default)]
pub(crate) struct Thumbnails {
    kept: Mutex<Kept>,
    fetching: Mutex<usize>,
    turn_freed: Condvar,
}

/// Oldest first. A short list searched in order: a few hundred at most.
#[derive(Default)]
struct Kept {
    entries: VecDeque<(i64, Option<Vec<u8>>)>,
    bytes: usize,
}

/// A download's turn, given back when dropped.
pub(crate) struct Turn<'a>(&'a Thumbnails);

impl Thumbnails {
    /// `Some(answer)` if this cover was fetched before: its bytes, or None
    /// for no cover.
    pub(crate) fn get(&self, cover_id: i64) -> Option<Option<Vec<u8>>> {
        let kept = lock(&self.kept);
        kept.entries
            .iter()
            .find(|(id, _)| *id == cover_id)
            .map(|(_, bytes)| bytes.clone())
    }

    /// Keeps an answer, letting go of the oldest ones past the limits.
    pub(crate) fn keep(&self, cover_id: i64, bytes: Option<Vec<u8>>) {
        let mut kept = lock(&self.kept);
        let size = bytes.as_ref().map_or(0, Vec::len);
        if size > MAX_KEPT_BYTES || kept.entries.iter().any(|(id, _)| *id == cover_id) {
            return;
        }
        while kept.entries.len() >= MAX_KEPT || kept.bytes + size > MAX_KEPT_BYTES {
            let Some((_, old)) = kept.entries.pop_front() else {
                break;
            };
            kept.bytes -= old.as_ref().map_or(0, Vec::len);
        }
        kept.bytes += size;
        kept.entries.push_back((cover_id, bytes));
    }

    /// Waits until fewer than `MAX_FETCHES` downloads are running.
    pub(crate) fn take_turn(&self) -> Turn<'_> {
        let mut fetching = lock(&self.fetching);
        while *fetching >= MAX_FETCHES {
            fetching = self
                .turn_freed
                .wait(fetching)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *fetching += 1;
        Turn(self)
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        *lock(&self.0.fetching) -= 1;
        self.0.turn_freed.notify_one();
    }
}

/// A panic while holding either lock leaves its contents usable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn answers_are_kept_within_the_limits() {
        let thumbs = Thumbnails::default();
        assert_eq!(thumbs.get(1), None);
        thumbs.keep(1, Some(vec![1; 10]));
        thumbs.keep(2, None);
        assert_eq!(thumbs.get(1), Some(Some(vec![1; 10])));
        assert_eq!(thumbs.get(2), Some(None), "no cover is an answer too");

        // Past the byte limit, the oldest go first.
        let big = MAX_KEPT_BYTES / 2;
        thumbs.keep(3, Some(vec![3; big]));
        thumbs.keep(4, Some(vec![4; big]));
        assert_eq!(thumbs.get(1), None);
        assert_eq!(thumbs.get(2), Some(None), "it took no room");
        assert!(thumbs.get(3).is_some() && thumbs.get(4).is_some());
        assert!(lock(&thumbs.kept).bytes <= MAX_KEPT_BYTES);
        // Something bigger than the whole budget isn't kept at all.
        thumbs.keep(5, Some(vec![5; MAX_KEPT_BYTES + 1]));
        assert_eq!(thumbs.get(5), None);
        assert!(thumbs.get(4).is_some());

        // And past the count limit.
        for id in 100..100 + MAX_KEPT as i64 {
            thumbs.keep(id, None);
        }
        assert_eq!(lock(&thumbs.kept).entries.len(), MAX_KEPT);
        assert_eq!(thumbs.get(4), None);
        assert_eq!(lock(&thumbs.kept).bytes, 0);
    }

    #[test]
    fn only_a_few_fetch_at_once() {
        let thumbs = Arc::new(Thumbnails::default());
        let peak = Arc::new(Mutex::new((0usize, 0usize))); // (now, most)
        let workers: Vec<_> = (0..10)
            .map(|_| {
                let (thumbs, peak) = (thumbs.clone(), peak.clone());
                std::thread::spawn(move || {
                    let _turn = thumbs.take_turn();
                    {
                        let mut p = peak.lock().unwrap();
                        p.0 += 1;
                        p.1 = p.1.max(p.0);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    peak.lock().unwrap().0 -= 1;
                })
            })
            .collect();
        for w in workers {
            w.join().unwrap();
        }
        let most = peak.lock().unwrap().1;
        assert!((1..=MAX_FETCHES).contains(&most), "{most} at once");
        assert_eq!(*lock(&thumbs.fetching), 0, "every turn was given back");
    }
}
