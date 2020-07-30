//! Utilities to write RDF graphs and datasets

use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use rio_api::formatter::{QuadsFormatter, TriplesFormatter};
use rio_turtle::{NQuadsFormatter, NTriplesFormatter, TriGFormatter, TurtleFormatter};
use rio_xml::{RdfXmlError, RdfXmlFormatter};
use std::io;
use std::io::Write;

/// A serializer for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Triples](https://www.w3.org/TR/n-triples/) (`GraphFormat::NTriples`)
/// * [Turtle](https://www.w3.org/TR/turtle/) (`GraphFormat::Turtle`)
/// * [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) (`GraphFormat::RdfXml`)
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = GraphSerializer::from_format(GraphFormat::NTriples).triple_writer(&mut buffer)?;
/// writer.write(&Triple {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into()
/// })?;
/// writer.finish()?;
///
///assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n".as_bytes());
/// # oxigraph::Result::Ok(())
/// ```
#[allow(missing_copy_implementations)]
pub struct GraphSerializer {
    format: GraphFormat,
}

impl GraphSerializer {
    pub fn from_format(format: GraphFormat) -> Self {
        Self { format }
    }

    /// Returns a `TripleWriter` allowing writing triples into the given `Write` implementation
    pub fn triple_writer<W: Write>(&self, writer: W) -> Result<TripleWriter<W>, io::Error> {
        Ok(TripleWriter {
            formatter: match self.format {
                GraphFormat::NTriples => TripleWriterKind::NTriples(NTriplesFormatter::new(writer)),
                GraphFormat::Turtle => TripleWriterKind::Turtle(TurtleFormatter::new(writer)),
                GraphFormat::RdfXml => {
                    TripleWriterKind::RdfXml(RdfXmlFormatter::new(writer).map_err(map_xml_err)?)
                }
            },
        })
    }
}

/// Allows writing triples.
/// Could be built using a `GraphSerializer`.
///
/// Warning: Do not forget to run the `finish` method to properly write the last bytes of the file.
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = GraphSerializer::from_format(GraphFormat::NTriples).triple_writer(&mut buffer)?;
/// writer.write(&Triple {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into()
/// })?;
/// writer.finish()?;
///
///assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n".as_bytes());
/// # oxigraph::Result::Ok(())
/// ```
#[must_use]
pub struct TripleWriter<W: Write> {
    formatter: TripleWriterKind<W>,
}

enum TripleWriterKind<W: Write> {
    NTriples(NTriplesFormatter<W>),
    Turtle(TurtleFormatter<W>),
    RdfXml(RdfXmlFormatter<W>),
}

impl<W: Write> TripleWriter<W> {
    pub fn write(&mut self, triple: &Triple) -> Result<(), io::Error> {
        match &mut self.formatter {
            TripleWriterKind::NTriples(formatter) => formatter.format(&triple.into())?,
            TripleWriterKind::Turtle(formatter) => formatter.format(&triple.into())?,
            TripleWriterKind::RdfXml(formatter) => {
                formatter.format(&triple.into()).map_err(map_xml_err)?
            }
        }
        Ok(())
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> Result<(), io::Error> {
        match self.formatter {
            TripleWriterKind::NTriples(formatter) => formatter.finish(),
            TripleWriterKind::Turtle(formatter) => formatter.finish()?,
            TripleWriterKind::RdfXml(formatter) => formatter.finish().map_err(map_xml_err)?,
        };
        Ok(())
    }
}

/// A serializer for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Quads](https://www.w3.org/TR/n-quads/) (`DatasetFormat::NQuads`)
/// * [TriG](https://www.w3.org/TR/trig/) (`DatasetFormat::TriG`)
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = DatasetSerializer::from_format(DatasetFormat::NQuads).quad_writer(&mut buffer)?;
/// writer.write(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into(),
/// })?;
/// writer.finish()?;
///
///assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
/// # oxigraph::Result::Ok(())
/// ```
#[allow(missing_copy_implementations)]
pub struct DatasetSerializer {
    format: DatasetFormat,
}

impl DatasetSerializer {
    pub fn from_format(format: DatasetFormat) -> Self {
        Self { format }
    }

    /// Returns a `QuadWriter` allowing writing triples into the given `Write` implementation
    pub fn quad_writer<W: Write>(&self, writer: W) -> Result<QuadWriter<W>, io::Error> {
        Ok(QuadWriter {
            formatter: match self.format {
                DatasetFormat::NQuads => QuadWriterKind::NQuads(NQuadsFormatter::new(writer)),
                DatasetFormat::TriG => QuadWriterKind::TriG(TriGFormatter::new(writer)),
            },
        })
    }
}

/// Allows writing triples.
/// Could be built using a `DatasetSerializer`.
///
/// Warning: Do not forget to run the `finish` method to properly write the last bytes of the file.
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = DatasetSerializer::from_format(DatasetFormat::NQuads).quad_writer(&mut buffer)?;
/// writer.write(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into(),
/// })?;
/// writer.finish()?;
///
///assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
/// # oxigraph::Result::Ok(())
/// ```
#[must_use]
pub struct QuadWriter<W: Write> {
    formatter: QuadWriterKind<W>,
}

enum QuadWriterKind<W: Write> {
    NQuads(NQuadsFormatter<W>),
    TriG(TriGFormatter<W>),
}

impl<W: Write> QuadWriter<W> {
    pub fn write(&mut self, triple: &Quad) -> Result<(), io::Error> {
        match &mut self.formatter {
            QuadWriterKind::NQuads(formatter) => formatter.format(&triple.into())?,
            QuadWriterKind::TriG(formatter) => formatter.format(&triple.into())?,
        }
        Ok(())
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> Result<(), io::Error> {
        match self.formatter {
            QuadWriterKind::NQuads(formatter) => formatter.finish(),
            QuadWriterKind::TriG(formatter) => formatter.finish()?,
        };
        Ok(())
    }
}

fn map_xml_err(e: RdfXmlError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e) //TODO: drop
}
