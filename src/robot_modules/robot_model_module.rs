#[cfg(not(target_arch = "wasm32"))]
use pyo3::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::robot_modules::robot_configuration_module::ContiguousChainMobilityMode;
use crate::utils::utils_errors::OptimaError;
use crate::utils::utils_robot::joint::{Joint};
use crate::utils::utils_robot::link::Link;
use crate::utils::utils_robot::urdf_joint::URDFJoint;
use crate::utils::utils_robot::urdf_link::URDFLink;
use crate::utils::utils_console::{optima_print, PrintColor, PrintMode};
use crate::utils::utils_files::optima_path::{load_object_from_json_string, OptimaAssetLocation, OptimaPathMatchingPattern, OptimaPathMatchingStopCondition, OptimaStemCellPath, RobotModuleJsonType};
use crate::utils::utils_generic_data_structures::SquareArray2D;
use crate::utils::utils_traits::{AssetSaveAndLoadable, SaveAndLoadable};

/// The `RobotModelModule` is the base description level for a robot.  It reflects component and
/// connectivity information about the robot as specified directly by the URDF.
/// Many other robot modules depend on this module.
///
/// The primary components of the RobotModelModule object are the lists of Link and Joint objects.
/// These links and joints are taken in top-down order from the URDF.  In other words, the URDF
/// is read from top to bottom and links and joints are stored in the order that they are seen.
/// Thus, the first link specified in the the URDF will have index 0, the second link specified in the
/// URDF will have index 1, and so on.  This order convention of links and joints is pervasive throughout
/// the whole library.
#[cfg_attr(not(target_arch = "wasm32"), pyclass, derive(Clone, Debug, Serialize, Deserialize))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen, derive(Clone, Debug, Serialize, Deserialize))]
pub struct RobotModelModule {
    robot_name: String,
    links: Vec<Link>,
    joints: Vec<Joint>,
    /// As specified by the URDF.
    world_link_idx: usize,
    /// As specified by the Optima robot model and configuration.  Most of the time, this will be
    /// equal to the world_link_idx.  However, if the robot has a mobile base, the robot_base_link_idx
    /// will be different than the world_link_idx.
    robot_base_link_idx: usize,
    link_tree_traversal_layers: Vec<Vec<usize>>,
    link_tree_max_depth: usize,
    preceding_actuated_joint_idxs: Vec<Option<usize>>,
    link_chains: SquareArray2D<Vec<usize>>,
    link_name_to_idx_hashmap: HashMap<String, usize>,
    joint_name_to_idx_hashmap: HashMap<String, usize>
}
impl RobotModelModule {
    /// Creates a new `RobotModelModule`.  The robot_name string is the name of the folder in the
    /// optima_assets/optima_robots directory.
    ///
    /// ## Example
    /// ```
    /// use optima::robot_modules::robot_model_module::RobotModelModule;
    /// let mut r = RobotModelModule::new_from_absolute_paths("ur5");
    /// ```
    pub fn new(robot_name: &str) -> Result<Self, OptimaError> {
        let load_result = Self::load_as_asset(OptimaAssetLocation::RobotModuleJson { robot_name: robot_name.to_string(), t: RobotModuleJsonType::ModelModule });
        if let Ok(load_result) = load_result { return Ok(load_result); }

        let mut joints = vec![];
        let mut links = vec![];

        let mut urdf_robot_joints = vec![];
        let mut urdf_robot_links = vec![];

        let mut link_name_to_idx_hashmap = HashMap::new();
        let mut joint_name_to_idx_hashmap = HashMap::new();

        let mut path_to_robot = OptimaStemCellPath::new_asset_path()?;
        path_to_robot.append_file_location(&OptimaAssetLocation::Robot {robot_name: robot_name.to_string()});
        if !path_to_robot.exists() {
            return Err(OptimaError::new_generic_error_str(format!("Robot directory for robot {} does not exist.", robot_name).as_str(), file!(), line!()))
        }
        let path_to_urdf_vec = path_to_robot.walk_directory_and_match(OptimaPathMatchingPattern::Extension("urdf".to_string()), OptimaPathMatchingStopCondition::First);
        if path_to_urdf_vec.is_empty() {
            return Err(OptimaError::new_generic_error_str(format!("Robot directory for robot {} does not contain a urdf.", robot_name).as_str(), file!(), line!()))
        }
        let path_to_urdf = path_to_urdf_vec[0].clone();
        let urdf_robot = path_to_urdf.load_urdf()?;
        for (i, j) in urdf_robot.joints.iter().enumerate() {
            joint_name_to_idx_hashmap.insert(j.name.clone(), i);
            joints.push(Joint::new(URDFJoint::new_from_urdf_joint(j), i));
            urdf_robot_joints.push(j);
        }
        for (i, l) in urdf_robot.links.iter().enumerate() {
            link_name_to_idx_hashmap.insert(l.name.clone(), i);
            links.push(Link::new(URDFLink::new_from_urdf_link(l), i));
            urdf_robot_links.push(l);
        }

        let num_links = links.len();

        let mut out_self = Self {
            robot_name: robot_name.to_string(),
            links,
            joints,
            world_link_idx: 0,
            robot_base_link_idx: 0,
            link_tree_traversal_layers: vec![],
            link_tree_max_depth: 0,
            preceding_actuated_joint_idxs: vec![],
            link_chains: SquareArray2D::new(num_links, false, None),
            link_name_to_idx_hashmap,
            joint_name_to_idx_hashmap
        };

        out_self.assign_all_link_connections_manual();
        out_self.assign_all_joint_connections_manual();
        out_self.set_world_link_idx_manual();
        out_self.set_link_tree_traversal_info();
        out_self.assign_all_link_chains();

        Ok(out_self)
    }
    fn assign_all_link_connections_manual(&mut self) {
        let l1 = self.links.len();
        let l2 = self.joints.len();

        for i in 0..l1 {
            for j in 0..l2 {
                if self.links[i].name() == self.joints[j].urdf_joint().child_link() {
                    let link_idx = self.get_link_idx_from_name( &self.joints[j].urdf_joint().parent_link().to_string() );
                    let joint_idx = self.get_joint_idx_from_name( &self.joints[j].name().to_string() );
                    self.links[i].set_preceding_link_idx( link_idx );
                    self.links[i].set_preceding_joint_idx( joint_idx );
                }

                if self.links[i].name() == self.joints[j].urdf_joint().parent_link() {
                    let link_idx = self.get_link_idx_from_name( &self.joints[j].urdf_joint().child_link().to_string() );
                    if link_idx.is_some() { self.links[i].add_child_link_idx(link_idx.unwrap()); }
                }
            }
        }
    }
    fn assign_all_joint_connections_manual(&mut self) {
        let l = self.joints.len();

        for i in 0..l {
            let link_idx = self.get_link_idx_from_name(  &self.joints[i].urdf_joint().parent_link().to_string()  );
            self.joints[i].set_preceding_link_idx(link_idx);
            let link_idx = self.get_link_idx_from_name(  &self.joints[i].urdf_joint().child_link().to_string()  );
            self.joints[i].set_child_link_idx(link_idx);
        }
    }
    fn assign_all_link_chains(&mut self) {
        let num_links = self.links.len();
        self.link_chains = SquareArray2D::new(num_links, false, None);

        for i in 0..num_links {
            for j in 0..num_links {
                self.assign_link_chain(i, j);
            }
        }
    }
    fn assign_link_chain(&mut self, from_idx: usize, to_idx: usize) {
        let mut out_vec = vec![to_idx];
        loop {
            let curr_link_idx = out_vec[0];
            let link = &self.links[curr_link_idx];
            let preceding_link_idx_option = link.preceding_link_idx();
            if preceding_link_idx_option.is_none() { return; }
            let preceding_link_idx = preceding_link_idx_option.unwrap();
            out_vec.insert(0, preceding_link_idx);
            if preceding_link_idx == from_idx {
                self.link_chains.adjust_data(|x| *x = out_vec.clone(), from_idx, to_idx).expect("error");
                return;
            }
        }
    }
    /// Returns the name of the robot
    pub fn robot_name(&self) -> &str {
        &self.robot_name
    }
    /// Returns the list of robot links.  Links are stored in top-down order from the URDF.
    pub fn links(&self) -> &Vec<Link> {
        &self.links
    }
    /// Returns the link by link idx.  If the index is too high for the given link, the
    /// function will return an error.
    pub fn get_link_by_idx(&self, idx: usize) -> Result<&Link, OptimaError> {
        if idx >= self.links().len() {
            return Err(OptimaError::new_idx_out_of_bound_error(idx, self.links().len(), file!(), line!()));
        }

        return Ok(&self.links[idx]);
    }
    /// Returns the list of robot joints.  Joints are stored in top-down order from the URDF.
    pub fn joints(&self) -> &Vec<Joint> {
        &self.joints
    }
    /// Returns the joint by link idx.  If the index is too high for the given joint, the
    /// function will return an error.
    pub fn get_joint_by_idx(&self, idx: usize) -> Result<&Joint, OptimaError> {
        if idx >= self.joints().len() {
            return Err(OptimaError::new_idx_out_of_bound_error(idx, self.joints().len(), file!(), line!()));
        }

        return Ok(&self.joints[idx]);
    }
    /// Returns the link index that represents the root global world as specified by the URDF.
    pub fn world_link_idx(&self) -> usize {
        self.world_link_idx
    }
    /// Returns the link index that represents the base of the robot.
    pub fn robot_base_link_idx(&self) -> usize {
        self.robot_base_link_idx
    }
    /// Returns the link tree traversal layers.  Each list in this ordered list specifies the links
    /// that are at a given layer in the robot's hierarchy.  For instance, suppose this diagram specifies
    /// a robot's link hierarchy:
    /// ```text
    ///         0
    ///        / \
    ///      2    3
    ///     / \    \
    ///    1  4     5
    /// ```
    /// Here, the numbers are link indices, and the lines represent joints that connect links together
    /// in the hierarchical chain.  In this case, the output of this function will be:
    /// \[\[0\], \[2,3\], \[1,4,5\]\].
    pub fn link_tree_traversal_layers(&self) -> &Vec<Vec<usize>> {
        &self.link_tree_traversal_layers
    }
    /// Returns the depth of the link tree traversal layers.  For instance, suppose this diagram specifies
    /// a robot's link hierarchy:
    /// ```text
    ///         0
    ///        / \
    ///      2    3
    ///     / \    \
    ///    1  4     5
    /// ```
    /// this function would return 2 (as it is zero-indexed and there are three layers in the tree).
    pub fn link_tree_max_depth(&self) -> usize {
        self.link_tree_max_depth
    }
    /// Returns the link tree traversal layer index that contains the given link.
    /// For instance,suppose this diagram specifies a robot's link hierarchy:
    /// ```text
    ///         0
    ///        / \
    ///      2    3
    ///     / \    \
    ///    1  4     5
    /// ```
    /// In this case, we would have the following:
    /// ## Example
    /// ```
    /// use optima::robot_modules::robot_model_module::RobotModelModule;
    /// let r = RobotModelModule::new_from_absolute_paths("fake_robot"); // pretend that fake_robot exists and its hierarchy matches the diagram.
    /// let l = r.get_link_tree_traveral_layer(2);
    /// assert!(l.unwrap() == 1);
    /// ```
    /// In this case, link 2 is in the "second" layer, which would return 1 from this function
    /// because it is zero-indexed.
    pub fn get_link_tree_traversal_layer(&self, link_idx: usize) -> Result<usize, OptimaError> {
        for (i, l) in self.link_tree_traversal_layers.iter().enumerate() {
            if l.contains(&link_idx) { return Ok(i); }
        }
        return Err(OptimaError::new_generic_error_str("link_idx not found in get_link_tree_traversal_layer()", file!(), line!()));
    }
    /// Returns the link index of the given link indices that is in the highest tree traveral layer.
    pub fn get_link_with_highest_tree_traversal_layer(&self, link_idxs: &Vec<usize>) -> Result<usize, OptimaError> {
        if link_idxs.len() == 1 { return Ok(link_idxs[0]); }
        if link_idxs.len() == 0 { return Err(OptimaError::new_generic_error_str(&format!("cannot have link_idxs with length 0 in get_link_with_highest_tree_traversal_layer()"), file!(), line!())); }

        let mut highest_layer = 0;
        let mut highest_layer_link_idx = 0;
        for l in link_idxs {
            let layer = self.get_link_tree_traversal_layer(*l)?;
            if layer >= highest_layer {
                highest_layer = layer;
                highest_layer_link_idx = *l;
            }
        }
        return Ok(highest_layer_link_idx);
    }
    /// Returns all links that are successors of link_idx in the kinematic chain (including link_idx itself).
    pub fn get_all_downstream_links(&self, link_idx: usize) -> Result<Vec<usize>, OptimaError> {
        let mut out_vec = vec![link_idx];

        let curr_link = self.get_link_by_idx(link_idx)?;
        let mut stack = curr_link.children_link_idxs().clone();

        loop {
            if stack.is_empty() { return Ok(out_vec) }

            let p = stack.remove(0);
            out_vec.push(p);
            let link = self.get_link_by_idx(p)?;
            for c in link.children_link_idxs() { stack.push(*c); }
        }
    }
    /// Function used during setup.  It is public since other modules may need to access it,
    /// but this should not need to be used by end users.
    fn set_world_link_idx_manual(&mut self) {
        let l = self.links.len();
        for i in 0..l {
            if self.links[i].preceding_link_idx().is_none() {
                self.world_link_idx = i;
                self.robot_base_link_idx = i;
                return;
            }
        }
    }
    /// Function used during setup.  It is public since other modules may need to access it,
    /// but this should not need to be used by end users.
    pub fn set_link_tree_traversal_info(&mut self) {
        self.link_tree_traversal_layers = vec![];
        self.link_tree_traversal_layers.push( vec![ self.world_link_idx ] );

        let num_links = self.links.len();
        let mut curr_layer = 1 as usize;
        loop {
            let mut change_on_this_loop = false;
            for i in 0..num_links {
                if self.links[i].preceding_link_idx().is_some() && self.links[i].present() {
                    if self.link_tree_traversal_layers[curr_layer - 1].contains(&self.links[i].preceding_link_idx().unwrap()) {
                        if self.link_tree_traversal_layers.len() == curr_layer { self.link_tree_traversal_layers.push( vec![] ); }

                        self.link_tree_traversal_layers[curr_layer].push( i );
                        change_on_this_loop = true;
                    }
                }
            }

            if change_on_this_loop {
                curr_layer += 1;
            } else {
                self.link_tree_max_depth = self.link_tree_traversal_layers.len();
                return;
            }
        }
    }
    /// Function used during setup.  It is public since other modules may need to access it,
    /// but this should not need to be used by end users.
    pub fn set_preceding_actuated_joint_idxs(&mut self) {
        self.preceding_actuated_joint_idxs = vec![];
        let num_links = self.links.len();
        for i in 0..num_links {
            let res = self.get_preceding_actuated_joint_idx(i);
            self.preceding_actuated_joint_idxs.push(res);
        }
    }
    /// Returns the closest preceding actuated joint index (i.e., a joint that has >0 DOFs) behind the
    /// given link.
    pub fn get_preceding_actuated_joint_idx(&self, link_idx: usize) -> Option<usize> {
        let links = &self.links;
        let joints = &self.joints;

        let mut curr_link_idx = link_idx;

        loop {
            let joint_idx = links[curr_link_idx].preceding_joint_idx();
            if joint_idx.is_none() { return None; }

            let joint_idx_unwrap = joint_idx.unwrap();
            let num_dofs = joints[joint_idx_unwrap].num_dofs();
            if num_dofs > 0 { return joint_idx; }

            let preceding_link_idx = joints[joint_idx_unwrap].preceding_link_idx();
            if preceding_link_idx.is_some() { return None; }

            curr_link_idx = preceding_link_idx.unwrap();
        }
    }
    /// Adds mobile base funtionality to the robot model.  This will likely be set automatically
    /// by RobotConfigurationModule, so there will very rarely be a need for the end user to
    /// call this function.
    pub fn add_contiguous_chain_link_and_joint(&mut self, base_of_chain_mobility_mode: &ContiguousChainMobilityMode, child_link_idx: usize) {
        match base_of_chain_mobility_mode {
            ContiguousChainMobilityMode::Static => { /* Do nothing */ }
            _ => {
                let new_link_idx = self.links().len();
                let new_joint_idx = self.joints().len();

                let new_link = Link::new_base_of_chain_link(new_link_idx, child_link_idx, new_joint_idx, self.world_link_idx);
                let new_joint = Joint::new_base_of_chain_connector_joint(base_of_chain_mobility_mode, new_joint_idx, new_link_idx, child_link_idx);

                self.link_name_to_idx_hashmap.insert(new_link.name().to_string(), new_link_idx);
                self.joint_name_to_idx_hashmap.insert(new_joint.name().to_string(), new_joint_idx);

                self.links.push(new_link);
                self.joints.push(new_joint);

                if child_link_idx == self.world_link_idx { self.robot_base_link_idx = new_link_idx; }

                self.links[child_link_idx].set_preceding_link_idx(Some(new_link_idx));

                self.set_link_tree_traversal_info();
                self.assign_all_link_chains();
            }
        }
    }
    /// Returns all links (by index) that have the given joint index as their closest preceding
    /// actuated joint index.
    pub fn get_all_link_idxs_with_given_preceding_actuated_joint_idx(&self, joint_idx: usize) -> Vec<usize> {
        let mut out_vec = vec![];
        for (i, a) in self.preceding_actuated_joint_idxs.iter().enumerate() {
            if a.is_some() && a.unwrap() == joint_idx {
                out_vec.push(i);
            }
        }
        out_vec
    }
    /// Returns link index by name.  If link with given name doesn't exist, this will return an error.
    pub fn get_link_idx_from_name(&self, link_name: &str) -> Option<usize> {
        let res = self.link_name_to_idx_hashmap.get(link_name);
        match res {
            None => { return None }
            Some(u) => { return Some(*u) }
        }
    }
    /// Returns joint index by name.  If joint with given name doesn't exist, this will return an error.
    pub fn get_joint_idx_from_name(&self, joint_name: &str) -> Option<usize> {
        let res = self.joint_name_to_idx_hashmap.get(joint_name);
        match res {
            None => { return None }
            Some(u) => { return Some(*u) }
        }
    }
    /// Prints the link tree traversal layers with link name descriptions.
    pub fn print_link_tree_traversal_layers_with_link_names(&self) {
        for i in 0..self.link_tree_max_depth {
            let l = self.link_tree_traversal_layers[i].len();
            // print!("layer {}: ", i);
            optima_print(&format!("layer {}: ", i), PrintMode::Print, PrintColor::Blue, true);
            for j in 0..l {
                let idx = self.link_tree_traversal_layers[i][j];
                optima_print(&format!("{}, ", self.links[idx].name()), PrintMode::Print, PrintColor::None, false);
            }
            optima_print("\n", PrintMode::Print, PrintColor::None, false);
        }
    }
    /// Sets given link as not present in the model.
    pub fn set_link_as_not_present(&mut self, link_idx: usize) -> Result<(), OptimaError> {
        if link_idx >= self.links().len() {
            return Err(OptimaError::new_idx_out_of_bound_error(link_idx, self.links().len(), file!(), line!()));
        }

        // The world link idx should never be inactive?  I'm still deciding on whether this should
        // be the case...
        if self.world_link_idx == link_idx { return Ok(()) }

        self.links[link_idx].set_present(false);

        let l = self.joints.len();
        for i in 0..l {
            let prec_option = self.joints[i].preceding_link_idx();
            if let Some(prec) = prec_option {
                if prec == link_idx {
                    self.set_joint_as_not_present(i)?;
                }
            }
        }

        Ok(())
    }
    pub fn set_joint_as_not_present(&mut self, joint_idx: usize) -> Result<(), OptimaError> {
        if joint_idx >= self.joints().len() {
            return Err(OptimaError::new_idx_out_of_bound_error(joint_idx, self.joints().len(), file!(), line!()));
        }

        self.joints[joint_idx].set_present(false);

        Ok(())
    }
    pub fn set_link_as_present(&mut self, link_idx: usize) -> Result<(), OptimaError> {
        if link_idx >= self.links().len() {
            return Err(OptimaError::new_idx_out_of_bound_error(link_idx, self.links().len(), file!(), line!()));
        }

        self.links[link_idx].set_present(true);

        let l = self.joints.len();
        for i in 0..l {
            let prec_option = self.joints[i].preceding_link_idx();
            if let Some(prec) = prec_option {
                if prec == link_idx {
                    self.set_joint_as_present(i)?;
                }
            }
        }

        Ok(())
    }
    pub fn set_joint_as_present(&mut self, joint_idx: usize) -> Result<(), OptimaError> {
        if joint_idx >= self.joints().len() {
            return Err(OptimaError::new_idx_out_of_bound_error(joint_idx, self.joints().len(), file!(), line!()));
        }

        self.joints[joint_idx].set_present(true);

        Ok(())
    }
    pub fn set_fixed_joint_sub_dof(&mut self, joint_idx: usize, joint_sub_idx: usize, fixed_value: Option<f64>) -> Result<(), OptimaError> {
        if joint_idx >= self.joints.len() {
            return Err(OptimaError::new_idx_out_of_bound_error(joint_idx, self.joints.len(), file!(), line!()));
        }

        return self.joints[joint_idx].set_fixed_joint_sub_dof(joint_sub_idx, fixed_value);
    }
    pub fn set_fixed_joint(&mut self, joint_idx: usize, fixed_value: Option<f64>) -> Result<(), OptimaError> {
        if joint_idx >= self.joints.len() {
            return Err(OptimaError::new_idx_out_of_bound_error(joint_idx, self.joints.len(), file!(), line!()));
        }

        let j = &mut self.joints[joint_idx];
        for d in 0..j.num_dofs() {
            j.set_fixed_joint_sub_dof(d, fixed_value)?;
        }
        Ok(())
    }
    pub fn get_link_chain(&self, from_link_idx: usize, to_link_idx: usize) -> Result<Option<&Vec<usize>>, OptimaError> {
        OptimaError::new_check_for_idx_out_of_bound_error(from_link_idx, self.links.len(), file!(), line!())?;
        OptimaError::new_check_for_idx_out_of_bound_error(to_link_idx, self.links.len(), file!(), line!())?;

        let res = self.link_chains.data_cell(from_link_idx, to_link_idx)?;
        return if res.is_empty() {
            Ok(None)
        } else {
            Ok(Some(res))
        }
    }
    pub fn print_links(&self) {
        for l in self.links.iter() {
            l.print_summary();
            print!("\n");
        }
    }
    pub fn print_joints(&self) {
        for j in self.joints.iter() {
            j.print_summary();
            print!("\n");
        }
    }
    pub fn print_summary(&self) {
        self.print_links();
        print!("\n");
        self.print_joints();
        print!("\n");
    }
}
impl SaveAndLoadable for RobotModelModule {
    type SaveType = Self;

    fn get_save_serialization_object(&self) -> Self::SaveType { self.clone() }
    fn load_from_path(path: &OptimaStemCellPath) -> Result<Self, OptimaError> {
        return path.load_object_from_json_file();
    }
    fn load_from_json_string(json_str: &str) -> Result<Self, OptimaError> where Self: Sized {
        let load: Self::SaveType = load_object_from_json_string(json_str)?;
        return Ok(load);
    }
}

/// Methods supported by python.
#[cfg(not(target_arch = "wasm32"))]
#[pymethods]
impl RobotModelModule {
    #[new]
    pub fn new_py(robot_name: &str) -> Self {
        return Self::new(robot_name).expect("error");
    }
    pub fn robot_name_py(&self) -> String { self.robot_name().to_string() }
    pub fn print_link_order_py(&self) {
        self.print_links();
    }
    pub fn print_link_tree_traversal_layers_with_link_names_py(&self) {
        self.print_link_tree_traversal_layers_with_link_names()
    }
    pub fn links_py(&self) -> Vec<Link> {
        self.links.clone()
    }
    pub fn joints_py(&self) -> Vec<Joint> { self.joints.clone() }
    pub fn world_link_idx_py(&self) -> usize {
        self.world_link_idx
    }
    pub fn link_tree_traversal_layers_py(&self) -> Vec<Vec<usize>> {
        self.link_tree_traversal_layers.clone()
    }
}

/// Methods supported by WASM.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl RobotModelModule {
    /*
    #[wasm_bindgen(constructor)]
    pub fn new_from_json_string_wasm(json_string: &str) -> Self {
        Self::new_load_from_json_string(json_string).expect("error")
    }
    */
    #[wasm_bindgen(constructor)]
    pub fn new_wasm(robot_name: &str) -> Self {
        Self::new(robot_name).expect("error")
    }
    pub fn robot_name_wasm(&self) -> String { self.robot_name.clone() }
    pub fn print_link_order_wasm(&self) {
        self.print_links();
    }
    pub fn print_link_tree_traversal_layers_with_link_names_wasm(&self) {
        self.print_link_tree_traversal_layers_with_link_names()
    }
}



