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
use log::error;
use log::info;
use pbfextractor::metrics;
use pbfextractor::metrics::Metric;
use pbfextractor::pbf;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::rc::Rc;
use std::time::SystemTime;

//------------------------------------------------------------------------------------------------//
// own modules

//------------------------------------------------------------------------------------------------//

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

//------------------------------------------------------------------------------------------------//

fn parse_cmdline<'a>() -> clap::ArgMatches<'a> {
    // arg: should output be zipped?
    let arg_zipped = clap::Arg::with_name("zipped")
        .short("z")
        .long("zipped")
        .help("If given, the output-file will be gzipped.");

    // arg: input-file: pbf
    let arg_in = clap::Arg::with_name("in")
        .short("i")
        .long("in")
        .value_name("PBF-FILE-PATH")
        .help("The path to the map-file being parsed.")
        .takes_value(true)
        .required(true);

    // arg: srtm-files
    let arg_srtm = clap::Arg::with_name("srtm")
        .long("srtm")
        .value_name("SRTM-PATH")
        .help("The path to the srtm-files.")
        .takes_value(true)
        .required(false);

    // arg: output-file
    let arg_out = clap::Arg::with_name("out")
        .short("o")
        .long("out")
        .value_name("FMI-FILE-PATH")
        .help("The path for the generated fmi-file.")
        .takes_value(true)
        .required(true);

    // arg: metrics
    // please find the filter (using these values) below
    let possible_values = vec![
        "chessboard",
        "distance",
        "gridx",
        "gridy",
        "random",
        "speed:car",
        "speed:fast-car",
        "speed:truck",
        "time:car",
        "time:fast-car",
        "time:truck",
    ];
    let arg_metrics = clap::Arg::with_name("metrics")
        .short("m")
        .long("metrics")
        .value_name("METRIC")
        .help("Metrics that should be calculated and added to the generated fmi-file.")
        .takes_value(true)
        .multiple(true)
        .possible_values(&possible_values)
        .required(true);

    // arg: internal metrics
    let tmp = &[
        "Metrics needed for other metrics, but not in the graph-file.",
        "Specifying both, metrics and internal metrics, just increases calculation time.",
    ]
    .join("\n");
    let arg_internal_only_metrics = clap::Arg::with_name("internal-only-metrics")
        .long("internal")
        .value_name("METRIC")
        .help(tmp)
        .takes_value(true)
        .multiple(true)
        .possible_values(&possible_values);

    // arg: quiet
    let tmp = &[
        "Doesn't log 'info', but only 'warn' and 'error'.",
        "The env-variable 'RUST_LOG' has precedence.",
    ]
    .join("\n");
    let arg_quiet = clap::Arg::with_name("quiet")
        .short("q")
        .long("quiet")
        .help(tmp);

    // all
    clap::App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(arg_zipped)
        .arg(arg_in)
        .arg(arg_srtm)
        .arg(arg_out)
        .arg(arg_metrics)
        .arg(arg_internal_only_metrics)
        .arg(arg_quiet)
        .get_matches()
}

fn setup_logging(quietly: bool) {
    let mut builder = env_logger::Builder::new();
    // minimum filter-level: `warn`
    builder.filter(None, log::LevelFilter::Warn);
    // if quiet logging: don't log `info` for the server and this repo
    if !quietly {
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

fn main() -> Result<(), ()> {
    let matches = parse_cmdline();
    setup_logging(matches.is_present("quiet"));

    // required args
    let input_path = matches.value_of("in").unwrap();
    let output_path = matches.value_of("out").unwrap();

    // data-structures needed for parsing metrics-args from user
    let mut chosen_metrics: Vec<&str> = matches.values_of("metrics").unwrap_or_default().collect();
    let chosen_internal_only_metrics: Vec<&str> = matches
        .values_of("internal-only-metrics")
        .unwrap_or_default()
        .collect();
    chosen_metrics.extend(chosen_internal_only_metrics.clone());

    info!("Chosen metrics: {:?}", chosen_metrics);
    info!(
        "Chosen internal-only-metrics: {:?}",
        chosen_internal_only_metrics
    );

    // needed as cloned-ref for some metrics
    let grid = metrics::Grid::new_ptr();
    let dist = Rc::new(metrics::Distance);
    let speed_car = Rc::new(metrics::CarSpeed);
    let speed_fast_car = Rc::new(metrics::FastCarSpeed);
    let speed_truck = Rc::new(metrics::TruckSpeed);
    // prepare metric-collections for pbf::Loader
    let mut tag_metrics: pbf::TagMetrics = pbf::TagMetrics::new();
    let mut node_metrics: pbf::NodeMetrics = pbf::NodeMetrics::new();
    let mut cost_metrics: pbf::CostMetrics = pbf::CostMetrics::new();
    let mut internal_only_metrics: pbf::InternalMetrics = pbf::InternalMetrics::new();
    // parse user-given metrics
    for metric_str in chosen_metrics {
        let metric_str = metric_str.trim().to_ascii_lowercase();
        let metric_name = match metric_str.as_ref() {
            // node-metrics
            "distance" => {
                node_metrics.push(dist.clone());
                dist.as_ref().name()
            }
            "gridx" => {
                let grid_x = Rc::new(metrics::GridX(grid.clone()));
                node_metrics.push(grid_x.clone());
                grid_x.as_ref().name()
            }
            "gridy" => {
                let grid_y = Rc::new(metrics::GridY(grid.clone()));
                node_metrics.push(grid_y.clone());
                grid_y.as_ref().name()
            }
            "chessboard" => {
                let chessboard = Rc::new(metrics::ChessBoard(grid.clone()));
                node_metrics.push(chessboard.clone());
                chessboard.as_ref().name()
            }
            // tag-metrics
            "speed:car" => {
                tag_metrics.push(speed_car.clone());
                speed_car.as_ref().name()
            }
            "speed:fast-car" => {
                tag_metrics.push(speed_fast_car.clone());
                speed_fast_car.as_ref().name()
            }
            "speed:truck" => {
                tag_metrics.push(speed_truck.clone());
                speed_truck.as_ref().name()
            }
            "random" => {
                let rand_weights = Rc::new(metrics::RandomWeights);
                tag_metrics.push(rand_weights.clone());
                rand_weights.as_ref().name()
            }
            // cost-metrics
            "time:car" => {
                let time_car = Rc::new(metrics::TravelTime::new(dist.clone(), speed_car.clone()));
                cost_metrics.push(time_car.clone());
                time_car.as_ref().name()
            }
            "time:fast-car" => {
                let time_fast_car = Rc::new(metrics::TravelTime::new(
                    dist.clone(),
                    speed_fast_car.clone(),
                ));
                cost_metrics.push(time_fast_car.clone());
                time_fast_car.as_ref().name()
            }
            "time:truck" => {
                let time_truck =
                    Rc::new(metrics::TravelTime::new(dist.clone(), speed_truck.clone()));
                cost_metrics.push(time_truck.clone());
                time_truck.as_ref().name()
            }
            // unsupported
            unsupported => {
                error!("Unsupported metric {}", unsupported);
                return Err(());
            }
        };

        // remember if metric is internal-only
        if chosen_internal_only_metrics.contains(&(metric_str.as_ref())) {
            internal_only_metrics.insert(metric_name);
        }
    }

    let l = pbf::Loader::new(
        input_path,
        matches.value_of("srtm"),
        metrics::CarEdgeFilter,
        tag_metrics,
        node_metrics,
        cost_metrics,
        internal_only_metrics,
        grid,
    );

    let output_file = File::create(&output_path).unwrap();
    let graph = BufWriter::new(output_file);
    if matches.is_present("zipped") {
        let graph = flate2::write::GzEncoder::new(graph, flate2::Compression::Best);
        write_graph(&l, graph);
    } else {
        write_graph(&l, graph);
    }

    Ok(())
}
