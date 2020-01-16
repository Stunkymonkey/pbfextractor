/*
 Pbfextractor creates graph files for the cycle-routing projects from pbf and srtm data
 Copyright (C) 2018  Florian Barth

 This program is free software: you can redistribute it and/or modify
 it under the terms of the GNU General Public License as published by
 the Free Software Foundation, either version 3 of the License, or
 (at your option) any later version.

 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU General Public License for more details.

 You should have received a copy of the GNU General Public License
 along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

//------------------------------------------------------------------------------------------------//
// other modules

use clap;
// use log::error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::rc::Rc;
use std::time::SystemTime;
use pbfextractor::{metrics, pbf};

//------------------------------------------------------------------------------------------------//
// own modules

//------------------------------------------------------------------------------------------------//

fn parse_cmdline<'a>() -> clap::ArgMatches<'a> {
    clap::App::new("PBF Extractor")
        .author("Florian Barth")
        .about("Extracts Graphs with multidimensional costs from PBF files")
        .args_from_usage(
            "-z          'saves graph gzipped'
             <PBF-FILE>   'PBF File to extract from'
             <SRTM>       'Directory with srtm files'
             <GRAPH>      'File to write graph to'",
        )
        .get_matches()
}

fn __setup_logging(verbosely: bool) {
    let mut builder = env_logger::Builder::new();
    // minimum filter-level: `warn`
    builder.filter(None, log::LevelFilter::Warn);
    // if verbose logging: log `info` for the server and this repo
    if verbosely {
        builder.filter(Some(env!("CARGO_PKG_NAME")), log::LevelFilter::Info);
    }
    // overwrite default with environment-variables
    if let Ok(filters) = std::env::var("RUST_LOG") {
        builder.parse_filters(&filters);
    }
    if let Ok(write_style) = std::env::var("RUST_LOG_STYLE") {
        builder.parse_write_style(&write_style);
    }
    // init
    builder.init();
}

fn main() {
    let matches = parse_cmdline();
    // setup_logging(matches.is_present("verbose"));

    let zip = matches.is_present("z");

    let pbf_input = matches
        .value_of("PBF-FILE")
        .expect("No PBF File to extract from");
    let srtm_input = matches.value_of("SRTM").expect("No srtm input file given");
    let output = matches.value_of("GRAPH").expect("No output file given");
    let grid = metrics::Grid::new_ptr();

    let dist = Rc::new(metrics::Distance);
    let car = Rc::new(metrics::CarSpeed);
    let fast_car = Rc::new(metrics::FastCarSpeed);
    let truck = Rc::new(metrics::TruckSpeed);

    let _grid_x = Rc::new(metrics::GridX(grid.clone()));
    let _grid_y = Rc::new(metrics::GridY(grid.clone()));
    let _chess = Rc::new(metrics::ChessBoard(grid.clone()));

    let _car_time = Rc::new(metrics::TravelTime::new(dist.clone(), car.clone()));
    let _fast_car_time = Rc::new(metrics::TravelTime::new(dist.clone(), fast_car.clone()));
    let _truck_time = Rc::new(metrics::TravelTime::new(dist.clone(), truck.clone()));

    let _random = Rc::new(metrics::RandomWeights);

    let internal_only_metrics: pbf::InternalMetrics = vec![].into_iter().collect();

    let tag_metrics: pbf::TagMetrics = vec![];
    let node_metrics: pbf::NodeMetrics = vec![dist];
    let cost_metrics: pbf::CostMetrics = vec![];

    let l = pbf::Loader::new(
        pbf_input,
        srtm_input,
        metrics::CarEdgeFilter,
        tag_metrics,
        node_metrics,
        cost_metrics,
        internal_only_metrics,
        grid,
    );

    let output_file = File::create(&output).unwrap();
    let graph = BufWriter::new(output_file);
    if zip {
        let graph = flate2::write::GzEncoder::new(graph, flate2::Compression::Best);
        write_graph(&l, graph);
    } else {
        write_graph(&l, graph);
    }
}

fn write_graph<T: metrics::EdgeFilter, W: Write>(l: &pbf::Loader<T>, mut graph: W) {
    let (nodes, edges) = l.load_graph();

    writeln!(&mut graph, "# Build by: pbfextractor").unwrap();
    writeln!(&mut graph, "# Build on: {:?}", SystemTime::now()).unwrap();
    write!(&mut graph, "# metrics: ").unwrap();

    for metric in l.metrics_indices.keys() {
        if l.internal_metrics.contains(metric) {
            continue;
        }
        write!(&mut graph, "{}, ", metric).unwrap();
    }

    write!(&mut graph, "\n\n").unwrap();

    writeln!(&mut graph, "{}", l.metric_count()).unwrap();
    writeln!(&mut graph, "{}", nodes.len()).unwrap();
    writeln!(&mut graph, "{}", edges.len()).unwrap();

    for (i, node) in nodes.iter().enumerate() {
        writeln!(
            &mut graph,
            "{} {} {} {} {} 0",
            i, node.osm_id, node.lat, node.long, node.height,
        )
        .unwrap();
    }
    for edge in &edges {
        write!(&mut graph, "{} {} ", edge.source, edge.dest).unwrap();
        for cost in &edge.costs(&l.metrics_indices, &l.internal_metrics) {
            write!(&mut graph, "{} ", cost).unwrap();
        }
        writeln!(&mut graph, "-1 -1").unwrap();
    }
    graph.flush().unwrap();
}