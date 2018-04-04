extern crate fst;
extern crate fst_levenshtein;
extern crate fst_regex;

use std::str;

use fst_levenshtein::Levenshtein;
use fst_regex::Regex;

use fst::{Automaton, IntoStreamer, Streamer};
use fst::raw::{Builder, Fst, Output};
use fst::set::{Set, OpBuilder};
use fst::map::Map;

static WORDS: &'static str = include_str!("../data/words-10000");

fn get_set() -> Set {
    Set::from_iter(WORDS.lines()).unwrap()
}

fn get_map() -> Map {
    Map::from_iter(WORDS.lines().enumerate().map(|(i, s)| (s, i as u64))).unwrap()
}

fn fst_set<I, S>(ss: I) -> Fst
        where I: IntoIterator<Item=S>, S: AsRef<[u8]> {
    let mut bfst = Builder::memory();
    let mut ss: Vec<Vec<u8>> =
        ss.into_iter().map(|s| s.as_ref().to_vec()).collect();
    ss.sort();
    for s in ss.iter().into_iter() {
        bfst.add(s).unwrap();
    }
    let fst = Fst::from_bytes(bfst.into_inner().unwrap()).unwrap();
    ss.dedup();
    assert_eq!(fst.len(), ss.len());
    fst
}

#[test]
fn regex_simple() {
    let set = fst_set(vec!["abc", "abd", "ayz", "za"]);
    let re = Regex::new("a[a-z]*").unwrap();
    let mut rdr = set.search(&re).ge("abd").lt("ax").into_stream();
    assert_eq!(rdr.next(), Some(("abd".as_bytes(), Output::zero())));
    assert!(rdr.next().is_none());
}

#[test]
fn levenshtein_simple() {
    let set = fst_set(vec!["woof", "wood", "banana"]);
    let q = Levenshtein::new("woog", 1).unwrap();
    let vs = set.search(&q).into_stream().into_byte_keys();
    assert_eq!(vs, vec!["wood".as_bytes(), "woof".as_bytes()]);
}

#[test]
fn levenshtein_unicode() {
    let set = fst_set(vec!["woof", "wood", "banana", "☃snowman☃"]);
    let q = Levenshtein::new("snoman", 3).unwrap();
    let vs = set.search(&q).into_stream().into_byte_keys();
    assert_eq!(vs, vec!["☃snowman☃".as_bytes()]);
}

#[test]
fn complement_small() {
    let keys = vec!["fa", "fo", "fob", "focus", "foo", "food", "foul"];
    let set = Set::from_iter(keys).unwrap();
    let lev = Levenshtein::new("foo", 1).unwrap();
    let stream = set.search(lev.complement()).into_stream();

    let keys = stream.into_strs().unwrap();
    assert_eq!(keys, vec!["fa", "focus", "foul"]);
}

#[test]
fn startswith_small() {
    let keys = vec![
        "", "cooing", "fa", "fo", "fob", "focus", "foo", "food", "foul",
        "fritter", "frothing",
    ];
    let set = Set::from_iter(keys).unwrap();
    let lev = Levenshtein::new("foo", 1).unwrap();
    let stream = set.search(lev.starts_with()).into_stream();

    let keys = stream.into_strs().unwrap();
    assert_eq!(keys, vec![
        "cooing", "fo", "fob", "focus", "foo", "food", "foul", "frothing",
    ]);
}

#[test]
fn intersection_small() {
    let keys = vec!["fa", "fo", "fob", "focus", "foo", "food", "foul"];
    let set = Set::from_iter(keys).unwrap();
    let lev = Levenshtein::new("foo", 1).unwrap();
    let reg = Regex::new("(..)*").unwrap();
    let stream = set.search(lev.intersection(reg)).into_stream();

    let keys = stream.into_strs().unwrap();
    assert_eq!(keys, vec!["fo", "food"]);
}

#[test]
fn union_small() {
    let keys = vec!["fa", "fo", "fob", "focus", "foo", "food", "foul"];
    let set = Set::from_iter(keys).unwrap();
    let lev = Levenshtein::new("foo", 1).unwrap();
    let reg = Regex::new("(..)*").unwrap();
    let stream = set.search(lev.union(reg)).into_stream();

    let keys = stream.into_strs().unwrap();
    assert_eq!(keys, vec!["fa", "fo", "fob", "foo", "food", "foul"]);
}

#[test]
fn intersection_large() {
    let set = get_set();
    let lev = Levenshtein::new("foo", 3).unwrap();
    let reg = Regex::new("(..)*").unwrap();
    let mut stream1 = set.search((&lev).intersection(&reg)).into_stream();
    let mut stream2 = OpBuilder::new()
        .add(set.search(&lev))
        .add(set.search(&reg))
        .intersection();
    while let Some(key1) = stream1.next() {
        assert_eq!(stream2.next(), Some(key1));
    }
    assert_eq!(stream2.next(), None);
}

#[test]
fn union_large() {
    let set = get_set();
    let lev = Levenshtein::new("foo", 3).unwrap();
    let reg = Regex::new("(..)*").unwrap();
    let mut stream1 = set.search((&lev).union(&reg)).into_stream();
    let mut stream2 = OpBuilder::new()
        .add(set.search(&lev))
        .add(set.search(&reg))
        .union();
    while let Some(key1) = stream1.next() {
        assert_eq!(stream2.next(), Some(key1));
    }
    assert_eq!(stream2.next(), None);
}

#[test]
fn find_set_in_map() {
    let map = get_map();
    let set = Set::from_iter(
        vec!["akhaioi", "asdfnotindict", "chedisthis", "conoceré", "etsplantatioets"].into_iter()
    ).unwrap();
    let mut stream = map.search(set.as_fst()).into_stream();
    let mut results = Vec::<(String, u64)>::new();
    while let Some(k) = stream.next() {
        results.push((str::from_utf8(k.0).unwrap().to_owned(), k.1));
    }
    // we should get back the position of four of the five words in the overall word list
    // and exclude the one that isn't in there
    assert_eq!(vec![
        ("akhaioi".to_owned(), 831),
        ("chedisthis".to_owned(), 1774),
        ("conoceré".to_owned(), 2008),
        ("etsplantatioets".to_owned(), 3068),
    ], results);
}