#![allow(clippy::same_name_method)]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::model::Quad;
use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef, TermRef};
use crate::storage::backend::{Reader, Transaction};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::storage::binary_encoder::LATEST_STORAGE_VERSION;
use crate::storage::binary_encoder::{
    decode_term, encode_term, encode_term_pair, encode_term_quad, encode_term_triple,
    write_gosp_quad, write_gpos_quad, write_gspo_quad, write_osp_quad, write_ospg_quad,
    write_pos_quad, write_posg_quad, write_spo_quad, write_spog_quad, write_term, QuadEncoding,
    WRITTEN_TERM_MAX_SIZE,
};
pub use crate::storage::error::{CorruptionError, LoaderError, SerializerError, StorageError};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::storage::numeric_encoder::Decoder;
use crate::storage::numeric_encoder::{insert_term, EncodedQuad, EncodedTerm, StrHash, StrLookup};
use backend::{ColumnFamily, ColumnFamilyDefinition, Db, Iter};

use gfa::parser::GFAParser;
use handlegraph::handlegraph::HandleGraph;
use handlegraph::path_position::PathPositionMap;
use handlegraph::pathhandlegraph::GraphPaths;
use handlegraph::{conversion::from_gfa, packedgraph::PackedGraph};
use std::str;

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::collections::VecDeque;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::collections::{HashMap, HashSet};
use std::error::Error;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::mem::{swap, take};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::path::{Path, PathBuf};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::sync::Mutex;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::{io, thread};

use self::storage_generator::StorageGenerator;

mod backend;
mod binary_encoder;
mod error;
pub mod numeric_encoder;
pub mod small_string;
mod storage_generator;
mod vg_vocab;

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
const DEFAULT_BULK_LOAD_BATCH_SIZE: usize = 1_000_000;

/// Low level storage primitives
#[derive(Clone)]
pub struct Storage {
    graph: PackedGraph,
    position_map: PathPositionMap,
    base: String,
}

impl Storage {
    pub fn new() -> Result<Self, StorageError> {
        let graph = PackedGraph::new();
        Ok(Self {
            graph: graph.clone(),
            position_map: PathPositionMap::index_paths(&graph),
            base: "https://example.org".to_owned(),
        })
    }

    pub fn from_str(gfa: &str) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_lines(gfa.lines().map(|s| s.as_bytes()))
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph: graph.clone(),
            position_map: PathPositionMap::index_paths(&graph),
            base: "https://example.org".to_owned(),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_file(path)
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph: graph.clone(),
            position_map: PathPositionMap::index_paths(&graph),
            base: "https://example.org".to_owned(),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_secondary(primary_path: &Path) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_file(primary_path)
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph: graph.clone(),
            position_map: PathPositionMap::index_paths(&graph),
            base: "https://example.org".to_owned(),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_persistent_secondary(
        primary_path: &Path,
        _secondary_path: &Path,
    ) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_file(primary_path)
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph: graph.clone(),
            position_map: PathPositionMap::index_paths(&graph),
            base: "https://example.org".to_owned(),
        })
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn open_read_only(path: &Path) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_file(path)
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph: graph.clone(),
            position_map: PathPositionMap::index_paths(&graph),
            base: "https://example.org".to_owned(),
        })
    }

    pub fn snapshot(&self) -> StorageReader {
        StorageReader::new(self.clone())
    }

    // pub fn transaction<'a, 'b: 'a, T, E: Error + 'static + From<StorageError>>(
    //     &'b self,
    //     f: impl Fn(StorageWriter<'a>) -> Result<T, E>,
    // ) -> Result<T, E> {
    //     // self.db.transaction(|transaction| {
    //     //     f(StorageWriter {
    //     //         buffer: Vec::new(),
    //     //         transaction,
    //     //         storage: self,
    //     //     })
    //     // })
    //     Err(StorageError::Io(std::io::Error::new(
    //         std::io::ErrorKind::Unsupported,
    //         "Transactions are currently not supported",
    //     )))
    // }
    #[cfg(not(target_family = "wasm"))]
    pub fn flush(&self) -> Result<(), StorageError> {
        Ok(())
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn compact(&self) -> Result<(), StorageError> {
        Ok(())
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn backup(&self, target_directory: &Path) -> Result<(), StorageError> {
        Ok(())
    }
}

pub struct StorageReader {
    // reader: Reader,
    // storage: Storage,
    generator: StorageGenerator,
}

impl StorageReader {
    pub fn new(storage: Storage) -> Self {
        Self {
            generator: StorageGenerator::new(storage),
        }
    }
    pub fn len(&self) -> Result<usize, StorageError> {
        let node_triples = self.generator.storage.graph.node_count() * 2;
        let path_triples = self.generator.storage.graph.path_count();
        let step_triples = 0;
        let edge_triples = self.generator.storage.graph.edge_count() * 2;
        Ok(node_triples + path_triples + step_triples + edge_triples)
    }

    pub fn is_empty(&self) -> Result<bool, StorageError> {
        Ok(self.generator.storage.graph.node_count() == 0)
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        // let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        // if quad.graph_name.is_default_graph() {
        //     write_spo_quad(&mut buffer, quad);
        //     Ok(self.reader.contains_key(&self.storage.dspo_cf, &buffer)?)
        // } else {
        //     write_gspo_quad(&mut buffer, quad);
        //     Ok(self.reader.contains_key(&self.storage.gspo_cf, &buffer)?)
        // }
        Ok(true)
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> ChainedDecodingQuadIterator {
        let graph_name = graph_name.expect("Graph name is given");
        self.generator
            .quads_for_pattern(subject, predicate, object, graph_name)
    }

    pub fn quads(&self) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(DecodingQuadIterator {
            terms: Box::new(Vec::new().into_iter()),
            encoding: QuadEncoding::Spog,
        })
        // ChainedDecodingQuadIterator::pair(self.dspo_quads(&[]), self.gspo_quads(&[]))
    }

    pub fn named_graphs(&self) -> DecodingGraphIterator {
        DecodingGraphIterator { terms: Vec::new() }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        // self.reader
        // .contains_key(&self.storage.graphs_cf, &encode_term(graph_name))
        Ok(true)
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(None)
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        Ok(true)
    }

    /// Validates that all the storage invariants held in the data
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn validate(&self) -> Result<(), StorageError> {
        Ok(())
    }

    /// Validates that all the storage invariants held in the data
    #[cfg(any(target_family = "wasm", not(feature = "rocksdb")))]
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    pub fn validate(&self) -> Result<(), StorageError> {
        Ok(()) // TODO
    }
}

pub struct ChainedDecodingQuadIterator {
    first: DecodingQuadIterator,
    second: Option<DecodingQuadIterator>,
}

impl ChainedDecodingQuadIterator {
    fn new(first: DecodingQuadIterator) -> Self {
        Self {
            first,
            second: None,
        }
    }

    fn pair(first: DecodingQuadIterator, second: DecodingQuadIterator) -> Self {
        Self {
            first,
            second: Some(second),
        }
    }
}

impl Iterator for ChainedDecodingQuadIterator {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(result) = self.first.next() {
            Some(result)
        } else if let Some(second) = self.second.as_mut() {
            second.next()
        } else {
            None
        }
    }
}

pub struct DecodingQuadIterator {
    terms: Box<dyn Iterator<Item = EncodedQuad>>,
    encoding: QuadEncoding,
}

impl Iterator for DecodingQuadIterator {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Result<EncodedQuad, StorageError>> {
        // if let Err(e) = self.iter.status() {
        //     return Some(Err(e));
        // }
        // let term = self.encoding.decode(self.iter.key()?);
        // self.iter.next();
        self.terms.next().map(|x| Ok(x))
    }
}

pub struct DecodingGraphIterator {
    terms: Vec<EncodedTerm>,
}

impl Iterator for DecodingGraphIterator {
    type Item = Result<EncodedTerm, StorageError>;

    fn next(&mut self) -> Option<Result<EncodedTerm, StorageError>> {
        // if let Err(e) = self.iter.status() {
        //     return Some(Err(e));
        // }
        // let term = self.encoding.decode(self.iter.key()?);
        // self.iter.next();
        self.terms.pop().map(|x| Ok(x))
    }
}

impl StrLookup for StorageReader {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        self.get_str(key)
    }
}

pub struct StorageWriter<'a> {
    buffer: Vec<u8>,
    transaction: Transaction<'a>,
    storage: &'a Storage,
}

impl<'a> StorageWriter<'a> {
    pub fn reader(&self) -> StorageReader {
        StorageReader::new(self.storage.clone())
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        Ok(true)
        //     let encoded = quad.into();
        //     self.buffer.clear();
        //     let result = if quad.graph_name.is_default_graph() {
        //         write_spo_quad(&mut self.buffer, &encoded);
        //         if self
        //             .transaction
        //             .contains_key_for_update(&self.storage.dspo_cf, &self.buffer)?
        //         {
        //             false
        //         } else {
        //             self.transaction
        //                 .insert_empty(&self.storage.dspo_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_pos_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.dpos_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_osp_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.dosp_cf, &self.buffer)?;

        //             self.insert_term(quad.subject.into(), &encoded.subject)?;
        //             self.insert_term(quad.predicate.into(), &encoded.predicate)?;
        //             self.insert_term(quad.object, &encoded.object)?;
        //             true
        //         }
        //     } else {
        //         write_spog_quad(&mut self.buffer, &encoded);
        //         if self
        //             .transaction
        //             .contains_key_for_update(&self.storage.spog_cf, &self.buffer)?
        //         {
        //             false
        //         } else {
        //             self.transaction
        //                 .insert_empty(&self.storage.spog_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_posg_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.posg_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_ospg_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.ospg_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_gspo_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.gspo_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_gpos_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.gpos_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_gosp_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.gosp_cf, &self.buffer)?;

        //             self.insert_term(quad.subject.into(), &encoded.subject)?;
        //             self.insert_term(quad.predicate.into(), &encoded.predicate)?;
        //             self.insert_term(quad.object, &encoded.object)?;

        //             self.buffer.clear();
        //             write_term(&mut self.buffer, &encoded.graph_name);
        //             if !self
        //                 .transaction
        //                 .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
        //             {
        //                 self.transaction
        //                     .insert_empty(&self.storage.graphs_cf, &self.buffer)?;
        //                 self.insert_graph_name(quad.graph_name, &encoded.graph_name)?;
        //             }
        //             true
        //         }
        //     };
        //     Ok(result)
    }

    pub fn insert_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        Ok(true)
        //     let encoded_graph_name = graph_name.into();

        //     self.buffer.clear();
        //     write_term(&mut self.buffer, &encoded_graph_name);
        //     let result = if self
        //         .transaction
        //         .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
        //     {
        //         false
        //     } else {
        //         self.transaction
        //             .insert_empty(&self.storage.graphs_cf, &self.buffer)?;
        //         self.insert_term(graph_name.into(), &encoded_graph_name)?;
        //         true
        //     };
        //     Ok(result)
    }

    // fn insert_term(
    //     &mut self,
    //     term: TermRef<'_>,
    //     encoded: &EncodedTerm,
    // ) -> Result<(), StorageError> {
    //     insert_term(term, encoded, &mut |key, value| self.insert_str(key, value))
    // }

    // fn insert_graph_name(
    //     &mut self,
    //     graph_name: GraphNameRef<'_>,
    //     encoded: &EncodedTerm,
    // ) -> Result<(), StorageError> {
    //     match graph_name {
    //         GraphNameRef::NamedNode(graph_name) => self.insert_term(graph_name.into(), encoded),
    //         GraphNameRef::BlankNode(graph_name) => self.insert_term(graph_name.into(), encoded),
    //         GraphNameRef::DefaultGraph => Ok(()),
    //     }
    // }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        // self.remove_encoded(&quad.into())
        Ok(true)
    }

    // fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<bool, StorageError> {
    //     self.buffer.clear();
    //     let result = if quad.graph_name.is_default_graph() {
    //         write_spo_quad(&mut self.buffer, quad);

    //         if self
    //             .transaction
    //             .contains_key_for_update(&self.storage.dspo_cf, &self.buffer)?
    //         {
    //             self.transaction
    //                 .remove(&self.storage.dspo_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_pos_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.dpos_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_osp_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.dosp_cf, &self.buffer)?;
    //             true
    //         } else {
    //             false
    //         }
    //     } else {
    //         write_spog_quad(&mut self.buffer, quad);

    //         if self
    //             .transaction
    //             .contains_key_for_update(&self.storage.spog_cf, &self.buffer)?
    //         {
    //             self.transaction
    //                 .remove(&self.storage.spog_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_posg_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.posg_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_ospg_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.ospg_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_gspo_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.gspo_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_gpos_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.gpos_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_gosp_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.gosp_cf, &self.buffer)?;
    //             true
    //         } else {
    //             false
    //         }
    //     };
    //     Ok(result)
    // }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
        // if graph_name.is_default_graph() {
        //     for quad in self.reader().quads_for_graph(&EncodedTerm::DefaultGraph) {
        //         self.remove_encoded(&quad?)?;
        //     }
        // } else {
        //     self.buffer.clear();
        //     write_term(&mut self.buffer, &graph_name.into());
        //     if self
        //         .transaction
        //         .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
        //     {
        //         // The condition is useful to lock the graph itself and ensure no quad is inserted at the same time
        //         for quad in self.reader().quads_for_graph(&graph_name.into()) {
        //             self.remove_encoded(&quad?)?;
        //         }
        //     }
        // }
        Ok(())
    }

    pub fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        // for quad in self.reader().quads_in_named_graph() {
        //     self.remove_encoded(&quad?)?;
        // }
        Ok(())
    }

    pub fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        // for quad in self.reader().quads() {
        //     self.remove_encoded(&quad?)?;
        // }
        Ok(())
    }

    pub fn remove_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        // self.remove_encoded_named_graph(&graph_name.into())
        Ok(true)
    }

    // fn remove_encoded_named_graph(
    //     &mut self,
    //     graph_name: &EncodedTerm,
    // ) -> Result<bool, StorageError> {
    //     self.buffer.clear();
    //     write_term(&mut self.buffer, graph_name);
    //     let result = if self
    //         .transaction
    //         .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
    //     {
    //         // The condition is done ASAP to lock the graph itself
    //         for quad in self.reader().quads_for_graph(graph_name) {
    //             self.remove_encoded(&quad?)?;
    //         }
    //         self.buffer.clear();
    //         write_term(&mut self.buffer, graph_name);
    //         self.transaction
    //             .remove(&self.storage.graphs_cf, &self.buffer)?;
    //         true
    //     } else {
    //         false
    //     };
    //     Ok(result)
    // }

    pub fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        // for graph_name in self.reader().named_graphs() {
        //     self.remove_encoded_named_graph(&graph_name?)?;
        // }
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), StorageError> {
        // for graph_name in self.reader().named_graphs() {
        //     self.remove_encoded_named_graph(&graph_name?)?;
        // }
        // for quad in self.reader().quads() {
        //     self.remove_encoded(&quad?)?;
        // }
        Ok(())
    }
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
#[must_use]
pub struct StorageBulkLoader {
    storage: Storage,
    hooks: Vec<Box<dyn Fn(u64)>>,
    num_threads: Option<usize>,
    max_memory_size: Option<usize>,
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
impl StorageBulkLoader {
    pub fn new(storage: Storage) -> Self {
        Self {
            storage,
            hooks: Vec::new(),
            num_threads: None,
            max_memory_size: None,
        }
    }

    pub fn with_num_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = Some(num_threads);
        self
    }

    pub fn with_max_memory_size_in_megabytes(mut self, max_memory_size: usize) -> Self {
        self.max_memory_size = Some(max_memory_size);
        self
    }

    pub fn on_progress(mut self, callback: impl Fn(u64) + 'static) -> Self {
        self.hooks.push(Box::new(callback));
        self
    }

    #[allow(clippy::trait_duplication_in_bounds)]
    pub fn load<EI, EO: From<StorageError> + From<EI>>(
        &self,
        quads: impl IntoIterator<Item = Result<Quad, EI>>,
    ) -> Result<(), EO> {
        let num_threads = self.num_threads.unwrap_or(2);
        if num_threads < 2 {
            return Err(
                StorageError::Other("The bulk loader needs at least 2 threads".into()).into(),
            );
        }
        let batch_size = if let Some(max_memory_size) = self.max_memory_size {
            max_memory_size * 1000 / num_threads
        } else {
            DEFAULT_BULK_LOAD_BATCH_SIZE
        };
        if batch_size < 10_000 {
            return Err(StorageError::Other(
                "The bulk loader memory bound is too low. It needs at least 100MB".into(),
            )
            .into());
        }
        let done_counter = Mutex::new(0);
        let mut done_and_displayed_counter = 0;
        thread::scope(|thread_scope| {
            let mut threads = VecDeque::with_capacity(num_threads - 1);
            let mut buffer = Vec::with_capacity(batch_size);
            for quad in quads {
                let quad = quad?;
                buffer.push(quad);
                if buffer.len() >= batch_size {
                    self.spawn_load_thread(
                        &mut buffer,
                        &mut threads,
                        thread_scope,
                        &done_counter,
                        &mut done_and_displayed_counter,
                        num_threads,
                        batch_size,
                    )?;
                }
            }
            self.spawn_load_thread(
                &mut buffer,
                &mut threads,
                thread_scope,
                &done_counter,
                &mut done_and_displayed_counter,
                num_threads,
                batch_size,
            )?;
            for thread in threads {
                map_thread_result(thread.join()).map_err(StorageError::Io)??;
                self.on_possible_progress(&done_counter, &mut done_and_displayed_counter)?;
            }
            Ok(())
        })
    }

    fn spawn_load_thread<'scope>(
        &'scope self,
        buffer: &mut Vec<Quad>,
        threads: &mut VecDeque<thread::ScopedJoinHandle<'scope, Result<(), StorageError>>>,
        thread_scope: &'scope thread::Scope<'scope, '_>,
        done_counter: &'scope Mutex<u64>,
        done_and_displayed_counter: &mut u64,
        num_threads: usize,
        batch_size: usize,
    ) -> Result<(), StorageError> {
        self.on_possible_progress(done_counter, done_and_displayed_counter)?;
        // We avoid to have too many threads
        if threads.len() >= num_threads {
            if let Some(thread) = threads.pop_front() {
                map_thread_result(thread.join()).map_err(StorageError::Io)??;
                self.on_possible_progress(done_counter, done_and_displayed_counter)?;
            }
        }
        let mut buffer_to_load = Vec::with_capacity(batch_size);
        swap(buffer, &mut buffer_to_load);
        let storage = &self.storage;
        threads.push_back(thread_scope.spawn(move || {
            FileBulkLoader::new(storage, batch_size).load(buffer_to_load, done_counter)
        }));
        Ok(())
    }

    fn on_possible_progress(
        &self,
        done: &Mutex<u64>,
        done_and_displayed: &mut u64,
    ) -> Result<(), StorageError> {
        let new_counter = *done
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Mutex poisoned"))?;
        let display_step = DEFAULT_BULK_LOAD_BATCH_SIZE as u64;
        if new_counter / display_step > *done_and_displayed / display_step {
            for hook in &self.hooks {
                hook(new_counter);
            }
        }
        *done_and_displayed = new_counter;
        Ok(())
    }
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
struct FileBulkLoader<'a> {
    storage: &'a Storage,
    id2str: HashMap<StrHash, Box<str>>,
    quads: HashSet<EncodedQuad>,
    triples: HashSet<EncodedQuad>,
    graphs: HashSet<EncodedTerm>,
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
impl<'a> FileBulkLoader<'a> {
    fn new(storage: &'a Storage, batch_size: usize) -> Self {
        Self {
            storage,
            id2str: HashMap::with_capacity(3 * batch_size),
            quads: HashSet::with_capacity(batch_size),
            triples: HashSet::with_capacity(batch_size),
            graphs: HashSet::default(),
        }
    }

    fn load(&mut self, quads: Vec<Quad>, counter: &Mutex<u64>) -> Result<(), StorageError> {
        self.encode(quads)?;
        let size = self.triples.len() + self.quads.len();
        //self.save()?;
        *counter
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Mutex poisoned"))? +=
            size.try_into().unwrap_or(u64::MAX);
        Ok(())
    }

    fn encode(&mut self, quads: Vec<Quad>) -> Result<(), StorageError> {
        for quad in quads {
            let encoded = EncodedQuad::from(quad.as_ref());
            if quad.graph_name.is_default_graph() {
                if self.triples.insert(encoded.clone()) {
                    self.insert_term(quad.subject.as_ref().into(), &encoded.subject)?;
                    self.insert_term(quad.predicate.as_ref().into(), &encoded.predicate)?;
                    self.insert_term(quad.object.as_ref(), &encoded.object)?;
                }
            } else if self.quads.insert(encoded.clone()) {
                self.insert_term(quad.subject.as_ref().into(), &encoded.subject)?;
                self.insert_term(quad.predicate.as_ref().into(), &encoded.predicate)?;
                self.insert_term(quad.object.as_ref(), &encoded.object)?;

                if self.graphs.insert(encoded.graph_name.clone()) {
                    self.insert_term(
                        match quad.graph_name.as_ref() {
                            GraphNameRef::NamedNode(n) => n.into(),
                            GraphNameRef::BlankNode(n) => n.into(),
                            GraphNameRef::DefaultGraph => {
                                return Err(CorruptionError::new(
                                    "Default graph this not the default graph",
                                )
                                .into())
                            }
                        },
                        &encoded.graph_name,
                    )?;
                }
            }
        }
        Ok(())
    }

    // fn save(&mut self) -> Result<(), StorageError> {
    //     let mut to_load = Vec::new();

    //     // id2str
    //     if !self.id2str.is_empty() {
    //         let mut id2str = take(&mut self.id2str)
    //             .into_iter()
    //             .map(|(k, v)| (k.to_be_bytes(), v))
    //             .collect::<Vec<_>>();
    //         id2str.sort_unstable();
    //         let mut id2str_sst = self.storage.db.new_sst_file()?;
    //         for (k, v) in id2str {
    //             id2str_sst.insert(&k, v.as_bytes())?;
    //         }
    //         to_load.push((&self.storage.id2str_cf, id2str_sst.finish()?));
    //     }

    //     if !self.triples.is_empty() {
    //         to_load.push((
    //             &self.storage.dspo_cf,
    //             self.build_sst_for_keys(
    //                 self.triples.iter().map(|quad| {
    //                     encode_term_triple(&quad.subject, &quad.predicate, &quad.object)
    //                 }),
    //             )?,
    //         ));
    //         to_load.push((
    //             &self.storage.dpos_cf,
    //             self.build_sst_for_keys(
    //                 self.triples.iter().map(|quad| {
    //                     encode_term_triple(&quad.predicate, &quad.object, &quad.subject)
    //                 }),
    //             )?,
    //         ));
    //         to_load.push((
    //             &self.storage.dosp_cf,
    //             self.build_sst_for_keys(
    //                 self.triples.iter().map(|quad| {
    //                     encode_term_triple(&quad.object, &quad.subject, &quad.predicate)
    //                 }),
    //             )?,
    //         ));
    //         self.triples.clear();
    //     }

    //     if !self.quads.is_empty() {
    //         to_load.push((
    //             &self.storage.graphs_cf,
    //             self.build_sst_for_keys(self.graphs.iter().map(encode_term))?,
    //         ));
    //         self.graphs.clear();

    //         to_load.push((
    //             &self.storage.gspo_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.graph_name,
    //                     &quad.subject,
    //                     &quad.predicate,
    //                     &quad.object,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.gpos_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.graph_name,
    //                     &quad.predicate,
    //                     &quad.object,
    //                     &quad.subject,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.gosp_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.graph_name,
    //                     &quad.object,
    //                     &quad.subject,
    //                     &quad.predicate,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.spog_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.subject,
    //                     &quad.predicate,
    //                     &quad.object,
    //                     &quad.graph_name,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.posg_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.predicate,
    //                     &quad.object,
    //                     &quad.subject,
    //                     &quad.graph_name,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.ospg_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.object,
    //                     &quad.subject,
    //                     &quad.predicate,
    //                     &quad.graph_name,
    //                 )
    //             }))?,
    //         ));
    //         self.quads.clear();
    //     }

    //     self.storage.db.insert_stt_files(&to_load)
    // }

    fn insert_term(
        &mut self,
        term: TermRef<'_>,
        encoded: &EncodedTerm,
    ) -> Result<(), StorageError> {
        insert_term(term, encoded, &mut |key, value| {
            self.id2str.entry(*key).or_insert_with(|| value.into());
            Ok(())
        })
    }

    // fn build_sst_for_keys(
    //     &self,
    //     values: impl Iterator<Item = Vec<u8>>,
    // ) -> Result<PathBuf, StorageError> {
    //     let mut values = values.collect::<Vec<_>>();
    //     values.sort_unstable();
    //     let mut sst = self.storage.db.new_sst_file()?;
    //     for value in values {
    //         sst.insert_empty(&value)?;
    //     }
    //     sst.finish()
    // }
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn map_thread_result<R>(result: thread::Result<R>) -> io::Result<R> {
    result.map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            if let Ok(e) = e.downcast::<&dyn std::fmt::Display>() {
                format!("A loader processed crashed with {e}")
            } else {
                "A loader processed crashed with and unknown error".into()
            },
        )
    })
}
