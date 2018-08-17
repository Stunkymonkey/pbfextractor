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
use osmpbfreader::{OsmObj, OsmPbfReader, Way};

use std::cmp::Ordering;
use std::fs::File;

pub struct Loader {
    pbf_path: String,
    srtm_path: String,
}

impl Loader {
    pub fn new(pbf_path: String, srtm_path: String) -> Loader {
        Loader {
            pbf_path: pbf_path,
            srtm_path: srtm_path,
        }
    }

    /// Loads the graph from a pbf file.
    pub fn load_graph(&self) -> (Vec<NodeInfo>, Vec<EdgeInfo>) {
        println!("Extracting data out of: {}", self.pbf_path);
        let fs = File::open(&self.pbf_path).unwrap();
        let mut reader = OsmPbfReader::new(fs);
        let obj_map = reader
            .get_objs_and_deps(|obj| {
                obj.tags().contains_key("highway") || obj.tags().contains("route", "bicycle")
            })
            .unwrap();

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for (_, obj) in &obj_map {
            match obj {
                OsmObj::Node(node) => {
                    let lat = (node.decimicro_lat as f64) / 10_000_000.0;
                    let lng = (node.decimicro_lon as f64) / 10_000_000.0;
                    nodes.push(NodeInfo::new(
                        node.id.0 as usize,
                        lat,
                        lng,
                        self.srtm(lat, lng),
                    ));
                }
                OsmObj::Way(w) => {
                    if self.is_not_for_bicycle(&w) {
                        continue;
                    }
                    self.process_way(&w, &mut edges, false);
                }
                OsmObj::Relation(r) => {
                    if !r.tags.contains("route", "bicycle") {
                        continue;
                    }
                    for reference in &r.refs {
                        let thing = &obj_map.get(&reference.member);
                        if let Some(OsmObj::Way(w)) = thing {
                            self.process_way(&w, &mut edges, true);
                        }
                    }
                }
            }
        }

        println!("Calculating distances and height differences on edges ");

        self.rename_node_ids_and_calculate_distance(&mut nodes, &mut edges);

        println!("Deleting duplicate edges");
        let edge_count = edges.len();

        edges.sort_by(|e1, e2| {
            let mut result = e1.source.cmp(&e2.source);
            if result == Ordering::Equal {
                result = e1.dest.cmp(&e2.dest);
            }
            if result == Ordering::Equal {
                let partial_result = e1.unsuitability.partial_cmp(&e2.unsuitability);
                result = if partial_result.is_some() {
                    partial_result.unwrap()
                } else {
                    Ordering::Equal
                }
            }
            if result == Ordering::Equal {
                let partial_result = e1.height.partial_cmp(&e2.height);
                result = if partial_result.is_some() {
                    partial_result.unwrap()
                } else {
                    Ordering::Equal
                }
            }
            if result == Ordering::Equal {
                let partial_result = e1.length.partial_cmp(&e2.length);
                result = if partial_result.is_some() {
                    partial_result.unwrap()
                } else {
                    Ordering::Equal
                }
            }
            return result;
        });

        edges.dedup();

        println!("Removed {} duplicated edges", edge_count - edges.len());

        let mut indices = ::std::collections::BTreeSet::new();
        for i in 1..edges.len() {
            let first = &edges[i - 1];
            let second = &edges[i];
            if !(first.source == second.source && first.dest == second.dest) {
                continue;
            }
            if first.length <= second.length && first.height <= second.height
                && first.unsuitability <= second.unsuitability
            {
                indices.insert(i);
            }
        }
        println!("removing {} dominated edges", indices.len());
        println!("len before {}", edges.len());
        edges = edges
            .into_iter()
            .enumerate()
            .filter(|(i, _)| {
                return !indices.contains(i);
            })
            .map(|(_, e)| {
                return e;
            })
            .collect();

        println!("len after {}", edges.len());
        return (nodes, edges);
    }

    fn determine_unsuitability(&self, way: &Way, bicycle_relation: bool) -> Unsuitability {
        let factor = if bicycle_relation { 0.5 } else { 1.0 };
        let bicycle_tag = way.tags.get("bicycle");
        if way.tags.get("cycleway").is_some()
            || bicycle_tag.is_some() && bicycle_tag != Some(&"no".to_string())
        {
            return 0.5 * factor;
        }

        let side_walk: Option<&str> = way.tags.get("sidewalk").map(String::as_ref);
        if side_walk == Some("yes") {
            return 1.0 * factor;
        }

        let street_type = way.tags.get("highway").map(String::as_ref);
        let unsuitability = match street_type {
            Some("primary") => 5.0,
            Some("primary_link") => 5.0,
            Some("secondary") => 4.0,
            Some("secondary_link") => 4.0,
            Some("tertiary") => 3.0,
            Some("tertiary_link") => 3.0,
            Some("road") => 3.0,
            Some("bridleway") => 3.0,
            Some("unclassified") => 2.0,
            Some("residential") => 2.0,
            Some("traffic_island") => 2.0,
            Some("living_street") => 1.0,
            Some("service") => 1.0,
            Some("track") => 1.0,
            Some("platform") => 1.0,
            Some("pedestrian") => 1.0,
            Some("path") => 1.0,
            Some("footway") => 1.0,
            Some("cycleway") => 0.5,
            _ => 6.0,
        };
        unsuitability * factor
    }

    fn process_way(&self, w: &Way, edges: &mut Vec<EdgeInfo>, bicycle_relation: bool) {
        let unsuitability = self.determine_unsuitability(&w, bicycle_relation);
        let is_one_way = self.is_one_way(&w);
        for (index, node) in w.nodes[0..(w.nodes.len() - 1)].iter().enumerate() {
            let edge = EdgeInfo::new(
                node.0 as NodeId,
                w.nodes[index + 1].0 as NodeId,
                1.1, // calculating length happens inside the graph
                0.0,
                unsuitability,
            );
            edges.push(edge);
            if !is_one_way {
                let edge = EdgeInfo::new(
                    w.nodes[index + 1].0 as NodeId,
                    node.0 as NodeId,
                    1.1, // calculating length happens inside the graph
                    0.0,
                    unsuitability,
                );
                edges.push(edge);
            }
        }
    }
    fn is_one_way(&self, way: &Way) -> bool {
        let one_way = way.tags.get("oneway").and_then(|s| s.parse().ok());
        match one_way {
            Some(rule) => rule,
            None => match way.tags.get("highway").map(|h| h == "motorway") {
                Some(rule) => rule,
                None => false,
            },
        }
    }

    fn is_not_for_bicycle(&self, way: &Way) -> bool {
        let bicycle_tag = way.tags.get("bicycle");
        if bicycle_tag == Some(&"no".to_string()) {
            return true;
        }
        if way.tags.get("cycleway").is_some()
            || bicycle_tag.is_some() && bicycle_tag != Some(&"no".to_string())
        {
            return false;
        }

        let street_type = way.tags.get("highway").map(String::as_ref);
        let side_walk: Option<&str> = way.tags.get("sidewalk").map(String::as_ref);
        let has_side_walk: bool = match side_walk {
            Some(s) => s != "no",
            None => false,
        };
        if has_side_walk {
            return false;
        }
        match street_type {
            Some("motorway")
            | Some("motorway_link")
            | Some("trunk")
            | Some("trunk_link")
            | Some("proposed")
            | Some("steps")
            | Some("elevator")
            | Some("corridor")
            | Some("raceway")
            | Some("rest_area")
            | Some("construction") => true,
            _ => false,
        }
    }

    fn rename_node_ids_and_calculate_distance(
        &self,
        nodes: &mut [NodeInfo],
        edges: &mut [EdgeInfo],
    ) {
        use std::collections::hash_map::HashMap;

        let map: HashMap<OsmNodeId, (usize, &NodeInfo)> =
            nodes.iter().enumerate().map(|n| (n.1.osm_id, n)).collect();
        for e in edges.iter_mut() {
            let (source_id, source) = map[&e.source];
            let (dest_id, dest) = map[&e.dest];
            e.source = source_id;
            e.dest = dest_id;
            e.length = self.haversine_distance(source, dest);
            let height_difference = dest.height - source.height;
            e.height = if height_difference > 0.0 {
                height_difference
            } else {
                0.0
            };
        }
    }

    /// Calculate the haversine distance. Adapted from https://github.com/georust/rust-geo
    pub fn haversine_distance(&self, a: &NodeInfo, b: &NodeInfo) -> Length {
        const EARTH_RADIUS: f64 = 6_371_007.2;

        let theta1 = a.lat.to_radians();
        let theta2 = b.lat.to_radians();
        let delta_theta = (b.lat - a.lat).to_radians();
        let delta_lambda = (b.long - a.long).to_radians();
        let a = (delta_theta / 2.0).sin().powi(2)
            + theta1.cos() * theta2.cos() * (delta_lambda / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        EARTH_RADIUS * c
    }

    fn srtm(&self, lat: Latitude, lng: Longitude) -> Height {
        use byteorder::{BigEndian, ReadBytesExt};
        use std::io::{Seek, SeekFrom};

        let second = 1.0 / 3600.0;

        let north = self.f64_to_whole_number(lat);
        let east = self.f64_to_whole_number(lng);

        let file_name = format!("/N{:02}E{:03}.hgt", north, east);

        let mut srtm_file = String::new();
        srtm_file.push_str(self.srtm_path.as_ref());
        srtm_file.push_str(&file_name);
        let mut f = match File::open(&srtm_file) {
            Ok(f) => f,
            Err(_) => {
                println!("could not find file: {}", file_name);
                return 0.0;
            }
        };

        let lat_offset = 3601.0 - lat.fract() / second;
        let lng_offset = lng.fract() / second;

        let lat_offset_floor = lat_offset.floor() as u64;
        let lat_offset_ceil = lat_offset.ceil() as u64;
        let long_offset_floor = lng_offset.floor() as u64;
        let long_offset_ceil = lng_offset.ceil() as u64;

        let mut read_offsets = |lat_offset: u64, long_offset: u64| -> f64 {
            f.seek(SeekFrom::Start(
                ((lat_offset - 1) * 3601 + (long_offset)) * 2,
            )).unwrap();

            f.read_i16::<BigEndian>()
                .expect(&format!("Reading failed at {}, {}", lat, lng)) as f64
        };

        let h1 = read_offsets(lat_offset_floor, long_offset_floor);
        let h2 = read_offsets(lat_offset_ceil, long_offset_floor);

        let h3 = read_offsets(lat_offset_floor, long_offset_ceil);
        let h4 = read_offsets(lat_offset_ceil, long_offset_ceil);

        let h1_weight = (1.0 - lat_offset.fract()) * (1.0 - lng_offset.fract());
        let h2_weight = lat_offset.fract() * (1.0 - lng_offset.fract());
        let h3_weight = (1.0 - lat_offset.fract()) * lng_offset.fract();
        let h4_weight = lat_offset.fract() * lng_offset.fract();

        (h1 * h1_weight + h2 * h2_weight + h3 * h3_weight + h4 * h4_weight)
    }

    fn f64_to_whole_number(&self, x: f64) -> u64 {
        x.trunc() as u64
    }
}

pub type NodeId = usize;
pub type OsmNodeId = usize;
pub type Latitude = f64;
pub type Longitude = f64;
pub type Length = f64;
pub type Height = f64;
pub type Unsuitability = f64;

pub struct NodeInfo {
    pub osm_id: OsmNodeId,
    pub lat: Latitude,
    pub long: Longitude,
    pub height: Height,
}

impl NodeInfo {
    pub fn new(osm_id: OsmNodeId, lat: Latitude, long: Longitude, height: Height) -> NodeInfo {
        NodeInfo {
            osm_id: osm_id,
            lat: lat,
            long: long,
            height: height,
        }
    }
}

pub struct EdgeInfo {
    pub source: NodeId,
    pub dest: NodeId,
    pub length: Length,
    pub height: Height,
    pub unsuitability: Unsuitability,
}

impl EdgeInfo {
    pub fn new(
        source: NodeId,
        dest: NodeId,
        length: Length,
        height: Height,
        unsuitability: Unsuitability,
    ) -> EdgeInfo {
        EdgeInfo {
            source: source,
            dest: dest,
            length: length,
            height: height,
            unsuitability: unsuitability,
        }
    }
}

impl PartialEq for EdgeInfo {
    fn eq(&self, rhs: &Self) -> bool {
        let mut equality = self.source == rhs.source && self.dest == rhs.dest
            && self.height == rhs.height
            && self.unsuitability == rhs.unsuitability;
        if equality {
            let partial_ord = self.length.partial_cmp(&rhs.length);
            equality = match partial_ord {
                Some(Ordering::Equal) => true,
                Some(_) => false,
                None => {
                    println!("PartialOrd evals to None");
                    true
                }
            }
        }

        equality
    }
}
