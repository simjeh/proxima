use std::ops::{Add, Index, Mul};
#[cfg(not(target_arch = "wasm32"))]
use pyo3::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use nalgebra::DVector;
use serde::{Serialize, Deserialize};
use crate::robot_modules::robot_configuration_generator_module::RobotConfigurationGeneratorModule;
use crate::robot_modules::robot_configuration_module::RobotConfigurationModule;
use crate::utils::utils_errors::OptimaError;
use crate::utils::utils_nalgebra::conversions::NalgebraConversions;
use crate::utils::utils_robot::joint::JointAxis;

/// The `RobotStateModule` organizes and operates over robot states.  "Robot states" are vectors
/// that contain scalar joint values for each joint axis in the robot model.
/// These objects are sometimes referred to as robot configurations or robot poses in the robotics literature,
/// but in this library, we will stick to the convention of referring to them as robot states.
///
/// The `RobotStateModule` has two primary fields:
/// - `ordered_dof_joint_axes`
/// - `ordered_joint_axes`
///
/// The `ordered_dof_joint_axes` field is a vector of `JointAxis` objects corresponding to the robot's
/// degrees of freedom (DOFs).  Note that this does NOT include any joint axes that have fixed values.
/// The number of robot degrees of freedom (DOFs) for a robot configuration is, thus, the length of
/// the `ordered_dof_joint_axes` vector (also accessible via the `num_dofs` field).
///
/// The `ordered_joint_axes` field is a vector of `JointAxis` objects corresponding to all axes
/// in the robot configuration.  Note that this DOES include joint axes, even if they have a fixed value.
/// The number of axes available in the robot configuration is accessible via the `num_axes` field.
/// Note that `num_dofs` <= `num_axes` for any robot configuration.
///
/// Note that neither the `ordered_dof_joint_axes` nor `ordered_joint_axes` vectors will include
/// joint axis objects on any joint that is listed as not present in the robot configuration.
///
/// These two differing views of joint axis lists (either including fixed axes or not) suggest two different
/// variants of robot states:
/// - A dof state
/// - A full state
///
/// A dof state only contains values for joint values that are free (not fixed), while the full state
/// includes joint values for ALL present joint axes (even if they are fixed).  A dof state is important
/// for operations such as optimization where only the free values are decision variables,
/// while a full state is important for operations such as forward kinematics where all present joint
/// axes need to somehow contribute to the model.
///
/// A dof state can be converted to a full state via the function `convert_dof_state_to_full_state`.
/// A full state can be converted to a dof state via the function `convert_full_state_to_dof_state`.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen, derive(Clone, Debug, Serialize, Deserialize))]
#[cfg_attr(not(target_arch = "wasm32"), pyclass, derive(Clone, Debug, Serialize, Deserialize))]
pub struct RobotStateModule {
    num_dofs: usize,
    num_axes: usize,
    ordered_dof_joint_axes: Vec<JointAxis>,
    ordered_joint_axes: Vec<JointAxis>,
    robot_configuration_module: RobotConfigurationModule,
    joint_idx_to_dof_state_idxs_mapping: Vec<Vec<usize>>,
    joint_idx_to_full_state_idxs_mapping: Vec<Vec<usize>>,
}
impl RobotStateModule {
    pub fn new(robot_configuration_module: RobotConfigurationModule) -> Self {
        let mut out_self = Self {
            num_dofs: 0,
            num_axes: 0,
            ordered_dof_joint_axes: vec![],
            ordered_joint_axes: vec![],
            robot_configuration_module,
            joint_idx_to_dof_state_idxs_mapping: vec![],
            joint_idx_to_full_state_idxs_mapping: vec![]
        };

        out_self.set_ordered_joint_axes();
        out_self.initialize_joint_idx_to_full_state_idxs();
        out_self.initialize_joint_idx_to_dof_state_idxs();
        out_self.num_dofs = out_self.ordered_dof_joint_axes.len();
        out_self.num_axes = out_self.ordered_joint_axes.len();

        return out_self;
    }

    pub fn new_from_names(robot_name: &str, configuration_name: Option<&str>) -> Result<Self, OptimaError> {
        let robot_configuration_module = RobotConfigurationGeneratorModule::new(robot_name)?.generate_configuration(configuration_name)?;
        return Ok(Self::new(robot_configuration_module));
    }

    fn set_ordered_joint_axes(&mut self) {
        for j in self.robot_configuration_module.robot_model_module().joints() {
            if j.active() {
                let joint_axes = j.joint_axes();
                for ja in joint_axes {
                    self.ordered_joint_axes.push(ja.clone());
                    if !ja.is_fixed() {
                        self.ordered_dof_joint_axes.push(ja.clone());
                    }
                }
            }
        }
    }

    fn initialize_joint_idx_to_dof_state_idxs(&mut self) {
        let mut out_vec = vec![];
        let num_joints = self.robot_configuration_module.robot_model_module().joints().len();
        for _ in 0..num_joints { out_vec.push(vec![]); }

        for (i, ja) in self.ordered_dof_joint_axes.iter().enumerate() {
            out_vec[ja.joint_idx()].push(i);
        }

        self.joint_idx_to_dof_state_idxs_mapping = out_vec;
    }

    fn initialize_joint_idx_to_full_state_idxs(&mut self) {
        let mut out_vec = vec![];
        let num_joints = self.robot_configuration_module.robot_model_module().joints().len();
        for _ in 0..num_joints { out_vec.push(vec![]); }

        for (i, ja) in self.ordered_joint_axes.iter().enumerate() {
            out_vec[ja.joint_idx()].push(i);
        }

        self.joint_idx_to_full_state_idxs_mapping = out_vec;
    }

    pub fn num_dofs(&self) -> usize {
        self.num_dofs
    }

    pub fn num_axes(&self) -> usize {
        self.num_axes
    }

    /// Returns joint axes in order (excluding fixed axes, thus only corresponding to degrees of freedom).
    pub fn ordered_dof_joint_axes(&self) -> &Vec<JointAxis> {
        &self.ordered_dof_joint_axes
    }

    /// Returns all joint axes in order (included fixed axes).
    pub fn ordered_joint_axes(&self) -> &Vec<JointAxis> {
        &self.ordered_joint_axes
    }

    /// Converts a dof state to a full state.
    pub fn convert_state_to_full_state(&self, state: &RobotState) -> Result<RobotState, OptimaError> {
        if state.len() != self.num_dofs {
            return Err(OptimaError::new_robot_state_vec_wrong_size_error("convert_dof_state_to_full_state", state.len(), self.num_dofs, file!(), line!()))
        }

        if state.robot_state_type() == &RobotStateType::DOF { return Ok(state.clone()); }

        let mut out_robot_state_vector = DVector::zeros(self.num_axes);

        let mut bookmark = 0 as usize;

        for (i, a) in self.ordered_joint_axes.iter().enumerate() {
            if a.is_fixed() {
                out_robot_state_vector[i] = a.fixed_value().unwrap();
            } else {
                out_robot_state_vector[i] = state[bookmark];
                bookmark += 1;
            }
        }

        return Ok(RobotState::new(out_robot_state_vector, RobotStateType::Full, self)?);
    }

    /// Converts a full state to a dof state.
    pub fn convert_state_to_dof_state(&self, state: &RobotState) -> Result<RobotState, OptimaError> {
        if state.len() != self.num_axes() {
            return Err(OptimaError::new_robot_state_vec_wrong_size_error("convert_full_state_to_dof_state", state.len(), self.num_axes, file!(), line!()))
        }

        if state.robot_state_type() == &RobotStateType::Full { return Ok(state.clone()); }

        let mut out_robot_state_vector = DVector::zeros(self.num_dofs);

        let mut bookmark = 0 as usize;

        for (i, a) in self.ordered_joint_axes.iter().enumerate() {
            if !a.is_fixed() {
                out_robot_state_vector[bookmark] = state[i];
                bookmark += 1;
            }
        }

        return Ok(RobotState::new(out_robot_state_vector, RobotStateType::DOF, self)?);
    }

    pub fn map_joint_idx_to_full_state_idxs(&self, joint_idx: usize) -> Result<&Vec<usize>, OptimaError> {
        if joint_idx >= self.joint_idx_to_full_state_idxs_mapping.len() {
            return Err(OptimaError::new_idx_out_of_bound_error(joint_idx, self.joint_idx_to_full_state_idxs_mapping.len(), file!(), line!()));
        }

        return Ok(&self.joint_idx_to_full_state_idxs_mapping[joint_idx]);
    }

    pub fn map_joint_idx_to_dof_state_idxs(&self, joint_idx: usize) -> Result<&Vec<usize>, OptimaError> {
        if joint_idx >= self.joint_idx_to_dof_state_idxs_mapping.len() {
            return Err(OptimaError::new_idx_out_of_bound_error(joint_idx, self.joint_idx_to_dof_state_idxs_mapping.len(), file!(), line!()));
        }

        return Ok(&self.joint_idx_to_dof_state_idxs_mapping[joint_idx]);
    }

    pub fn spawn_robot_state(&self, state: DVector<f64>, robot_state_type: RobotStateType) -> Result<RobotState, OptimaError> {
        return RobotState::new(state, robot_state_type, self);
    }

    pub fn spawn_robot_state_try_auto_type(&self, state: DVector<f64>) -> Result<RobotState, OptimaError> {
        return RobotState::new_try_auto_type(state, self);
    }
}

/// Python implementations.
#[cfg(not(target_arch = "wasm32"))]
#[pymethods]
impl RobotStateModule {
    #[new]
    pub fn new_py(robot_name: &str, configuration_name: Option<&str>) -> RobotStateModule {
        return Self::new_from_names(robot_name, configuration_name).expect("error");
    }

    pub fn convert_state_to_full_state_py(&self, state: Vec<f64>) -> Vec<f64> {
        let robot_state = self.spawn_robot_state_try_auto_type(NalgebraConversions::vec_to_dvector(&state)).expect("error");
        let res = self.convert_state_to_full_state(&robot_state).expect("error");
        return NalgebraConversions::dvector_to_vec(&res.state);
    }

    pub fn convert_state_to_dof_state_py(&self, state: Vec<f64>) -> Vec<f64> {
        let robot_state = self.spawn_robot_state_try_auto_type(NalgebraConversions::vec_to_dvector(&state)).expect("error");
        let res = self.convert_state_to_dof_state(&robot_state).expect("error");
        return NalgebraConversions::dvector_to_vec(&res.state);
    }

    pub fn num_dofs_py(&self) -> usize { self.num_dofs() }

    pub fn num_axes_py(&self) -> usize {
        self.num_axes()
    }
}

/// WASM implementations.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl RobotStateModule {
    #[wasm_bindgen(constructor)]
    pub fn new_wasm(robot_name: String, configuration_name: Option<String>) -> RobotStateModule {
        return match configuration_name {
            None => { Self::new_from_names(&robot_name, None).expect("error") }
            Some(c) => { Self::new_from_names(&robot_name, Some(&c)).expect("error") }
        }
    }

    pub fn convert_state_to_full_state_wasm(&self, state: Vec<f64>) -> Vec<f64> {
        let robot_state = self.spawn_robot_state_try_auto_type(NalgebraConversions::vec_to_dvector(&state)).expect("error");
        let res = self.convert_state_to_full_state(&robot_state).expect("error");
        return NalgebraConversions::dvector_to_vec(&res.state);
    }

    pub fn convert_state_to_dof_state_wasm(&self, state: Vec<f64>) -> Vec<f64> {
        let robot_state = self.spawn_robot_state_try_auto_type(NalgebraConversions::vec_to_dvector(&state)).expect("error");
        let res = self.convert_state_to_dof_state(&robot_state).expect("error");
        return NalgebraConversions::dvector_to_vec(&res.state);
    }

    pub fn num_dofs_wasm(&self) -> usize { self.num_dofs() }

    pub fn num_axes_wasm(&self) -> usize {
        self.num_axes()
    }
}

/// "Robot states" are vectors that contain scalar joint values for each joint axis in the robot model.
/// These objects are sometimes referred to as robot configurations or robot poses in the robotics literature,
/// but in this library, we will stick to the convention of referring to them as robot states.
///
/// A `RobotState` object contains the vector of joint angles in the field `state`, as well as a
/// state type (either DOF or Full).
///
/// A DOF state only contains values for joint values that are free (not fixed), while the Full state
/// includes joint values for ALL present joint axes (even if they are fixed).  A dof state is important
/// for operations such as optimization where only the free values are decision variables,
/// while a full state is important for operations such as forward kinematics where all present joint
/// axes need to somehow contribute to the model.
///
/// The library will ensure that mathematical operations (additions, scalar multiplication, etc) can
/// only occur over robot states of the same type.  Conversions between DOF and Full states can be done
/// via the `RobotStateModule`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RobotState {
    state: DVector<f64>,
    robot_state_type: RobotStateType
}
impl RobotState {
    pub fn new(state: DVector<f64>, robot_state_type: RobotStateType, robot_state_module: &RobotStateModule) -> Result<Self, OptimaError> {
        match robot_state_type {
            RobotStateType::DOF => {
                if robot_state_module.num_dofs() != state.len() {
                    return Err(OptimaError::new_robot_state_vec_wrong_size_error("RobotState::new", state.len(), robot_state_module.num_dofs(), file!(), line!()));
                }
            }
            RobotStateType::Full => {
                if robot_state_module.num_axes() != state.len() {
                    return Err(OptimaError::new_robot_state_vec_wrong_size_error("RobotState::new", state.len(), robot_state_module.num_axes(), file!(), line!()));
                }
            }
        }

        Ok(Self {
            state,
            robot_state_type
        })
    }
    pub fn new_try_auto_type(state: DVector<f64>, robot_state_module: &RobotStateModule) -> Result<Self, OptimaError> {
        return if robot_state_module.num_axes() == state.len() {
            Ok(Self::new_unchecked(state, RobotStateType::Full))
        } else if robot_state_module.num_dofs() == state.len() {
            Ok(Self::new_unchecked(state, RobotStateType::DOF))
        } else {
            Err(OptimaError::new_generic_error_str(&format!("Could not successfully make an auto \
            RobotState in try_new_auto_type().  The given state length was {} while either {} or {} was required.",
                                                            state.len(), robot_state_module.num_axes(), robot_state_module.num_dofs()),
                                                   file!(),
                                                   line!()))
        }
    }
    fn new_unchecked(state: DVector<f64>, robot_state_type: RobotStateType) -> Self {
        Self {
            state,
            robot_state_type
        }
    }
    pub fn new_dof_state(state: DVector<f64>, robot_state_module: &RobotStateModule) -> Result<Self, OptimaError> {
        Self::new(state, RobotStateType::DOF, robot_state_module)
    }
    pub fn new_full_state(state: DVector<f64>, robot_state_module: &RobotStateModule) -> Result<Self, OptimaError> {
        Self::new(state, RobotStateType::Full, robot_state_module)
    }
    pub fn state(&self) -> &DVector<f64> {
        &self.state
    }
    pub fn robot_state_type(&self) -> &RobotStateType {
        &self.robot_state_type
    }
    pub fn len(&self) -> usize {
        return self.state.len();
    }
}
impl Add for RobotState {
    type Output = Result<RobotState, OptimaError>;

    fn add(self, rhs: Self) -> Self::Output {
        if &self.robot_state_type != &rhs.robot_state_type {
            return Err(OptimaError::new_generic_error_str(&format!("Tried to add robot states of different types ({:?} + {:?}).", self.robot_state_type(), rhs.robot_state_type()), file!(), line!()));
        }
        return Ok(RobotState::new_unchecked(self.state() + rhs.state(), self.robot_state_type.clone()))
    }
}
impl Mul<RobotState> for f64 {
    type Output = RobotState;

    fn mul(self, rhs: RobotState) -> Self::Output {
        return RobotState::new_unchecked(self * rhs.state(), rhs.robot_state_type.clone());
    }
}
impl Index<usize> for RobotState {
    type Output = f64;

    fn index(&self, index: usize) -> &Self::Output {
        return &self.state[index];
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RobotStateType {
    DOF,
    Full
}