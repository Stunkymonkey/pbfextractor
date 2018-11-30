use super::pbf::NodeInfo;
use osmpbfreader::Tags;
use std::collections::HashMap;

#[derive(Debug)]
pub enum MetricError {
    UnknownMetric,
    NonFiniteTime(f64, f64),
}

pub type MetricResult = Result<f64, MetricError>;

pub trait Metric {
    fn name(&self) -> &'static str;
}

pub trait TagMetric: Metric {
    fn calc(&self, tags: &Tags) -> MetricResult;
}

pub trait NodeMetric: Metric {
    fn calc(&self, source: &NodeInfo, target: &NodeInfo) -> MetricResult;
}

pub trait CostMetric: Metric {
    fn calc(&self, costs: &[f64], map: &HashMap<&'static str, usize>) -> MetricResult;
}

pub struct CarSpeed;

impl Metric for CarSpeed {
    fn name(&self) -> &'static str {
        "CarSpeed"
    }
}

impl TagMetric for CarSpeed {
    fn calc(&self, tags: &Tags) -> MetricResult {
        let max_speed = tags.get("maxspeed").and_then(|s| s.parse().ok());
        let speed = match max_speed {
            Some(s) if s > 0.0 => s,
            _ => {
                let street_type = tags.get("highway").map(String::as_ref);
                match street_type {
                    Some("motorway") | Some("trunk") => 130.0,
                    Some("primary") => 100.0,
                    Some("secondary") | Some("trunk_link") => 80.0,
                    Some("motorway_link")
                    | Some("primary_link")
                    | Some("secondary_link")
                    | Some("tertiary")
                    | Some("tertiary_link") => 70.0,
                    Some("service") => 30.0,
                    Some("living_street") => 5.0,
                    _ => 50.0,
                }
            }
        };
        Ok(speed)
    }
}

pub struct Distance;
impl Metric for Distance {
    fn name(&self) -> &'static str {
        "distance"
    }
}
impl NodeMetric for Distance {
    fn calc(&self, source: &NodeInfo, target: &NodeInfo) -> MetricResult {
        const EARTH_RADIUS: f64 = 6_371_007.2;
        let theta1 = source.lat.to_radians();
        let theta2 = target.lat.to_radians();
        let delta_theta = (target.lat - source.lat).to_radians();
        let delta_lambda = (target.long - source.long).to_radians();
        let a = (delta_theta / 2.0).sin().powi(2)
            + theta1.cos() * theta2.cos() * (delta_lambda / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        Ok(EARTH_RADIUS * c)
    }
}

pub struct TravelTime;
impl Metric for TravelTime {
    fn name(&self) -> &'static str {
        "TravelTime"
    }
}

impl CostMetric for TravelTime {
    fn calc(&self, costs: &[f64], map: &HashMap<&'static str, usize>) -> MetricResult {
        let dist_index = map.get(Distance.name()).ok_or(MetricError::UnknownMetric)?;
        let speed_index = map.get(CarSpeed.name()).ok_or(MetricError::UnknownMetric)?;
        let dist = costs[*dist_index];
        let speed = costs[*speed_index];
        let time = dist * 360.0 / speed;
        if time.is_finite() {
            Ok(time)
        } else {
            Err(MetricError::NonFiniteTime(dist, speed))
        }
    }
}

pub struct HeightAscent;
impl Metric for HeightAscent {
    fn name(&self) -> &'static str {
        "HeightAscent"
    }
}
impl NodeMetric for HeightAscent {
    fn calc(&self, source: &NodeInfo, target: &NodeInfo) -> MetricResult {
        let height_diff = target.height - source.height;
        if height_diff > 0.0 {
            Ok(height_diff)
        } else {
            Ok(0.0)
        }
    }
}

pub struct BicycleUnsuitability;
impl Metric for BicycleUnsuitability {
    fn name(&self) -> &'static str {
        "BicycleUnsuitability"
    }
}
impl TagMetric for BicycleUnsuitability {
    fn calc(&self, tags: &Tags) -> MetricResult {
        let bicycle_tag = tags.get("bicycle");
        if tags.get("cycleway").is_some()
            || bicycle_tag.is_some() && bicycle_tag != Some(&"no".to_string())
        {
            return Ok(0.5);
        }

        let side_walk: Option<&str> = tags.get("sidewalk").map(String::as_ref);
        if side_walk == Some("yes") {
            return Ok(1.0);
        }

        let street_type = tags.get("highway").map(String::as_ref);
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
        Ok(unsuitability)
    }
}
