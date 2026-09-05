// SPDX-License-Identifier: GPL-3.0-or-later

use crate::unit::Unit;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug)]
pub struct Graph {
    units: BTreeMap<String, Unit>,
    follows: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    Missing {
        unit: String,
        needs: String,
    },
    Cycle(Vec<String>),
    Incompatible {
        unit: String,
        with: String,
        pulled_in_by: String,
    },
    Unknown(String),
}

impl fmt::Display for Trouble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trouble::Missing { unit, needs } => write!(
                f,
                "{unit} requires {needs}, and there is no unit by that name.\n\
                 either add {needs}.toml or move it to wants, which tolerates it being absent"
            ),
            Trouble::Cycle(path) => write!(
                f,
                "these units are ordered in a circle, so none of them can go first:\n  {}",
                path.join(" -> ")
            ),
            Trouble::Incompatible {
                unit,
                with,
                pulled_in_by,
            } => write!(
                f,
                "{unit} conflicts with {with}, but both were pulled in by {pulled_in_by}.\n\
                 one of them has to go, or the conflict is wrong"
            ),
            Trouble::Unknown(name) => write!(f, "there is no unit called {name}"),
        }
    }
}

impl Graph {
    pub fn build(units: Vec<Unit>) -> Result<Graph, Trouble> {
        let units: BTreeMap<String, Unit> =
            units.into_iter().map(|u| (u.name.clone(), u)).collect();

        for unit in units.values() {
            for needed in &unit.requires {
                if !units.contains_key(needed) {
                    return Err(Trouble::Missing {
                        unit: unit.name.clone(),
                        needs: needed.clone(),
                    });
                }
            }
        }

        let mut follows: BTreeMap<String, BTreeSet<String>> =
            units.keys().map(|n| (n.clone(), BTreeSet::new())).collect();

        for unit in units.values() {
            for earlier in &unit.after {
                if units.contains_key(earlier) {
                    follows
                        .get_mut(&unit.name)
                        .expect("every unit has an entry")
                        .insert(earlier.clone());
                }
            }
            for later in &unit.before {
                if let Some(edges) = follows.get_mut(later) {
                    edges.insert(unit.name.clone());
                }
            }
        }

        let graph = Graph { units, follows };
        graph.find_a_cycle()?;
        Ok(graph)
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.units.keys()
    }

    pub fn get(&self, name: &str) -> Option<&Unit> {
        self.units.get(name)
    }

    pub fn pull_in(&self, roots: &[String]) -> Result<BTreeSet<String>, Trouble> {
        let mut chosen = BTreeSet::new();
        let mut pending: Vec<String> = Vec::new();

        for root in roots {
            if !self.units.contains_key(root) {
                return Err(Trouble::Unknown(root.clone()));
            }
            pending.push(root.clone());
        }

        while let Some(name) = pending.pop() {
            if !chosen.insert(name.clone()) {
                continue;
            }
            let unit = self.units.get(&name).expect("chosen names exist");
            for needed in unit.needs() {
                if self.units.contains_key(needed) {
                    pending.push(needed.clone());
                }
            }
        }

        self.check_nothing_conflicts(&chosen)?;
        Ok(chosen)
    }

    fn check_nothing_conflicts(&self, chosen: &BTreeSet<String>) -> Result<(), Trouble> {
        for name in chosen {
            let unit = self.units.get(name).expect("chosen names exist");
            for enemy in &unit.conflicts {
                if chosen.contains(enemy) {
                    return Err(Trouble::Incompatible {
                        unit: name.clone(),
                        with: enemy.clone(),
                        pulled_in_by: self.who_asked_for(enemy, chosen),
                    });
                }
            }
        }
        Ok(())
    }

    fn who_asked_for(&self, name: &str, chosen: &BTreeSet<String>) -> String {
        for other in chosen {
            let unit = self.units.get(other).expect("chosen names exist");
            if unit.needs().any(|n| n == name) {
                return other.clone();
            }
        }
        name.to_owned()
    }

    pub fn start_order(&self, roots: &[String]) -> Result<Vec<String>, Trouble> {
        Ok(self.waves(roots)?.into_iter().flatten().collect())
    }

    pub fn stop_order(&self, roots: &[String]) -> Result<Vec<String>, Trouble> {
        let mut order = self.start_order(roots)?;
        order.reverse();
        Ok(order)
    }

    pub fn waves(&self, roots: &[String]) -> Result<Vec<Vec<String>>, Trouble> {
        let chosen = self.pull_in(roots)?;
        let mut waiting: BTreeMap<&String, BTreeSet<&String>> = chosen
            .iter()
            .map(|name| {
                let blockers = self
                    .follows
                    .get(name)
                    .expect("every unit has an entry")
                    .iter()
                    .filter(|earlier| chosen.contains(*earlier))
                    .collect();
                (name, blockers)
            })
            .collect();

        let mut waves = Vec::new();
        while !waiting.is_empty() {
            let ready: Vec<&String> = waiting
                .iter()
                .filter(|(_, blockers)| blockers.is_empty())
                .map(|(name, _)| *name)
                .collect();

            if ready.is_empty() {
                return Err(Trouble::Cycle(self.find_a_cycle_among(
                    waiting.keys().map(|n| (*n).clone()).collect(),
                )));
            }

            for name in &ready {
                waiting.remove(*name);
            }
            for blockers in waiting.values_mut() {
                for name in &ready {
                    blockers.remove(*name);
                }
            }
            waves.push(ready.into_iter().cloned().collect());
        }
        Ok(waves)
    }

    pub fn ready(&self, chosen: &BTreeSet<String>, already_up: &BTreeSet<String>) -> Vec<String> {
        chosen
            .iter()
            .filter(|name| !already_up.contains(*name))
            .filter(|name| {
                self.follows
                    .get(*name)
                    .expect("every unit has an entry")
                    .iter()
                    .filter(|earlier| chosen.contains(*earlier))
                    .all(|earlier| already_up.contains(earlier))
            })
            .cloned()
            .collect()
    }

    fn find_a_cycle(&self) -> Result<(), Trouble> {
        let everything: BTreeSet<String> = self.units.keys().cloned().collect();
        let mut settled = BTreeSet::new();
        let mut path = Vec::new();

        for name in &everything {
            if let Some(cycle) = self.walk(name, &everything, &mut settled, &mut path) {
                return Err(Trouble::Cycle(cycle));
            }
        }
        Ok(())
    }

    fn find_a_cycle_among(&self, stuck: BTreeSet<String>) -> Vec<String> {
        let mut settled = BTreeSet::new();
        let mut path = Vec::new();
        for name in &stuck {
            if let Some(cycle) = self.walk(name, &stuck, &mut settled, &mut path) {
                return cycle;
            }
        }
        stuck.into_iter().collect()
    }

    fn walk(
        &self,
        name: &String,
        within: &BTreeSet<String>,
        settled: &mut BTreeSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        if settled.contains(name) {
            return None;
        }
        if let Some(from) = path.iter().position(|seen| seen == name) {
            let mut cycle: Vec<String> = path[from..].to_vec();
            cycle.push(name.clone());
            return Some(cycle);
        }

        path.push(name.clone());
        for earlier in self.follows.get(name).expect("every unit has an entry") {
            if within.contains(earlier)
                && let Some(cycle) = self.walk(earlier, within, settled, path)
            {
                return Some(cycle);
            }
        }
        path.pop();
        settled.insert(name.clone());
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{Graph, Trouble};
    use crate::unit::{Kind, Restart, Unit};
    use std::collections::BTreeSet;

    fn unit(name: &str) -> Unit {
        Unit {
            name: name.to_owned(),
            description: name.to_owned(),
            requires: Vec::new(),
            wants: Vec::new(),
            after: Vec::new(),
            before: Vec::new(),
            conflicts: Vec::new(),
            kind: Kind::Simple,
            start: "/bin/true".to_owned(),
            stop: None,
            restart: Restart::Never,
            restart_delay: 1,
            restart_limit: 5,
            start_timeout: 30,
            stop_timeout: 10,
            directory: None,
            environment: Vec::new(),
        }
    }

    fn named(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    fn roots(names: &[&str]) -> Vec<String> {
        named(names)
    }

    #[test]
    fn requires_pulls_the_other_unit_in() {
        let mut a = unit("a");
        a.requires = named(&["b"]);
        let graph = Graph::build(vec![a, unit("b")]).expect("b exists");
        let chosen = graph.pull_in(&roots(&["a"])).expect("a is known");
        assert_eq!(chosen, BTreeSet::from(["a".to_owned(), "b".to_owned()]));
    }

    #[test]
    fn wants_pulls_in_too_but_forgives_an_absence() {
        let mut a = unit("a");
        a.wants = named(&["b", "ghost"]);
        let graph = Graph::build(vec![a, unit("b")]).expect("a missing want is fine");
        let chosen = graph.pull_in(&roots(&["a"])).expect("a is known");
        assert_eq!(chosen, BTreeSet::from(["a".to_owned(), "b".to_owned()]));
    }

    #[test]
    fn a_missing_requirement_is_refused_with_a_way_out() {
        let mut a = unit("a");
        a.requires = named(&["ghost"]);
        let trouble = Graph::build(vec![a]).expect_err("ghost does not exist");
        assert_eq!(
            trouble,
            Trouble::Missing {
                unit: "a".to_owned(),
                needs: "ghost".to_owned()
            }
        );
        assert!(
            trouble.to_string().contains("move it to wants"),
            "{trouble}"
        );
    }

    #[test]
    fn requiring_something_does_not_order_against_it() {
        let mut a = unit("a");
        a.requires = named(&["b"]);
        let graph = Graph::build(vec![a, unit("b")]).expect("valid");
        let waves = graph.waves(&roots(&["a"])).expect("valid");
        assert_eq!(waves, vec![named(&["a", "b"])]);
    }

    #[test]
    fn after_puts_the_other_one_first() {
        let mut a = unit("a");
        a.requires = named(&["b"]);
        a.after = named(&["b"]);
        let graph = Graph::build(vec![a, unit("b")]).expect("valid");
        assert_eq!(
            graph.start_order(&roots(&["a"])).expect("valid"),
            named(&["b", "a"])
        );
    }

    #[test]
    fn before_is_the_same_edge_seen_from_the_other_side() {
        let mut b = unit("b");
        b.before = named(&["a"]);
        let mut a = unit("a");
        a.requires = named(&["b"]);
        let graph = Graph::build(vec![a, b]).expect("valid");
        assert_eq!(
            graph.start_order(&roots(&["a"])).expect("valid"),
            named(&["b", "a"])
        );
    }

    #[test]
    fn ordering_against_a_unit_that_is_not_starting_does_not_hold_us_back() {
        let mut a = unit("a");
        a.after = named(&["b"]);
        let graph = Graph::build(vec![a, unit("b")]).expect("valid");
        assert_eq!(
            graph.start_order(&roots(&["a"])).expect("valid"),
            named(&["a"])
        );
    }

    #[test]
    fn stopping_is_the_start_order_backwards() {
        let mut a = unit("a");
        a.requires = named(&["b"]);
        a.after = named(&["b"]);
        let graph = Graph::build(vec![a, unit("b")]).expect("valid");
        let up = graph.start_order(&roots(&["a"])).expect("valid");
        let down = graph.stop_order(&roots(&["a"])).expect("valid");
        assert_eq!(down, up.iter().rev().cloned().collect::<Vec<_>>());
        assert_eq!(down, named(&["a", "b"]));
    }

    #[test]
    fn independent_units_land_in_the_same_wave() {
        let mut root = unit("root");
        root.requires = named(&["left", "right"]);
        root.after = named(&["left", "right"]);
        let graph = Graph::build(vec![root, unit("left"), unit("right")]).expect("valid");
        let waves = graph.waves(&roots(&["root"])).expect("valid");
        assert_eq!(waves, vec![named(&["left", "right"]), named(&["root"])]);
    }

    #[test]
    fn a_chain_gets_one_wave_each() {
        let mut a = unit("a");
        a.requires = named(&["b"]);
        a.after = named(&["b"]);
        let mut b = unit("b");
        b.requires = named(&["c"]);
        b.after = named(&["c"]);
        let graph = Graph::build(vec![a, b, unit("c")]).expect("valid");
        let waves = graph.waves(&roots(&["a"])).expect("valid");
        assert_eq!(waves, vec![named(&["c"]), named(&["b"]), named(&["a"])]);
    }

    #[test]
    fn a_circle_of_two_is_caught_and_written_out_whole() {
        let mut a = unit("a");
        a.after = named(&["b"]);
        let mut b = unit("b");
        b.after = named(&["a"]);
        let trouble = Graph::build(vec![a, b]).expect_err("a circle cannot be ordered");
        let Trouble::Cycle(path) = &trouble else {
            panic!("expected a cycle, got {trouble}");
        };
        assert_eq!(path.first(), path.last(), "a cycle has to close: {path:?}");
        assert_eq!(path.len(), 3, "a and b and back to the first: {path:?}");
        assert!(trouble.to_string().contains("->"), "{trouble}");
    }

    #[test]
    fn a_longer_circle_reports_every_step() {
        let mut a = unit("a");
        a.after = named(&["b"]);
        let mut b = unit("b");
        b.after = named(&["c"]);
        let mut c = unit("c");
        c.after = named(&["a"]);
        let trouble = Graph::build(vec![a, b, c]).expect_err("still a circle");
        let Trouble::Cycle(path) = &trouble else {
            panic!("expected a cycle, got {trouble}");
        };
        assert_eq!(path.len(), 4, "three units and back: {path:?}");
        for name in ["a", "b", "c"] {
            assert!(
                path.contains(&name.to_owned()),
                "{name} missing from {path:?}"
            );
        }
    }

    #[test]
    fn a_circle_drawn_with_before_is_still_a_circle() {
        let mut a = unit("a");
        a.after = named(&["b"]);
        let mut b = unit("b");
        b.before = named(&["b"]);
        b.after = named(&["a"]);
        let trouble = Graph::build(vec![a, b]).expect_err("a circle either way");
        assert!(matches!(trouble, Trouble::Cycle(_)), "{trouble}");
    }

    #[test]
    fn two_units_that_conflict_cannot_both_be_pulled_in() {
        let mut a = unit("a");
        a.requires = named(&["b", "c"]);
        let mut b = unit("b");
        b.conflicts = named(&["c"]);
        let graph = Graph::build(vec![a, b, unit("c")]).expect("the graph itself is fine");
        let trouble = graph
            .pull_in(&roots(&["a"]))
            .expect_err("b and c cannot coexist");
        assert!(matches!(trouble, Trouble::Incompatible { .. }), "{trouble}");
        assert!(trouble.to_string().contains("pulled in by a"), "{trouble}");
    }

    #[test]
    fn units_that_conflict_but_are_not_both_wanted_are_left_alone() {
        let mut b = unit("b");
        b.conflicts = named(&["c"]);
        let graph = Graph::build(vec![b, unit("c")]).expect("valid");
        assert!(graph.pull_in(&roots(&["b"])).is_ok());
    }

    #[test]
    fn asking_for_a_unit_that_does_not_exist_says_so() {
        let graph = Graph::build(vec![unit("a")]).expect("valid");
        let trouble = graph.pull_in(&roots(&["ghost"])).expect_err("no such unit");
        assert_eq!(trouble, Trouble::Unknown("ghost".to_owned()));
    }

    #[test]
    fn readiness_matches_what_the_waves_say() {
        let mut a = unit("a");
        a.requires = named(&["b"]);
        a.after = named(&["b"]);
        let graph = Graph::build(vec![a, unit("b")]).expect("valid");
        let chosen = graph.pull_in(&roots(&["a"])).expect("valid");

        let nothing_up = BTreeSet::new();
        assert_eq!(graph.ready(&chosen, &nothing_up), named(&["b"]));

        let b_up = BTreeSet::from(["b".to_owned()]);
        assert_eq!(graph.ready(&chosen, &b_up), named(&["a"]));

        let both_up = BTreeSet::from(["a".to_owned(), "b".to_owned()]);
        assert!(graph.ready(&chosen, &both_up).is_empty());
    }

    #[test]
    fn the_order_does_not_wander_between_runs() {
        let build = || {
            let mut root = unit("root");
            root.requires = named(&["x", "y", "z"]);
            root.after = named(&["x", "y", "z"]);
            Graph::build(vec![root, unit("x"), unit("y"), unit("z")]).expect("valid")
        };
        let once = build().start_order(&roots(&["root"])).expect("valid");
        let twice = build().start_order(&roots(&["root"])).expect("valid");
        assert_eq!(once, twice);
        assert_eq!(once, named(&["x", "y", "z", "root"]));
    }

    #[test]
    fn the_shipped_examples_form_a_graph_that_resolves() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("units");
        let mut loaded = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("units/ ships with the crate") {
            let path = entry.expect("a readable entry").path();
            let file = path
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            let Some(name) = Unit::name_from_file(&file) else {
                continue;
            };
            let text = std::fs::read_to_string(&path).expect("a readable unit");
            loaded.push(Unit::parse(name, &file, &text).unwrap_or_else(|e| panic!("{e}")));
        }
        let names: Vec<String> = loaded.iter().map(|u| u.name.clone()).collect();
        let graph =
            Graph::build(loaded).unwrap_or_else(|e| panic!("the examples do not build: {e}"));
        for name in &names {
            graph
                .start_order(std::slice::from_ref(name))
                .unwrap_or_else(|e| panic!("{name} cannot be started: {e}"));
        }
    }

    #[test]
    fn the_graph_remembers_the_units_it_was_given() {
        let graph = Graph::build(vec![unit("a"), unit("b")]).expect("valid");
        assert_eq!(
            graph.names().cloned().collect::<Vec<_>>(),
            named(&["a", "b"])
        );
        assert!(graph.get("a").is_some());
        assert!(graph.get("ghost").is_none());
    }
}
