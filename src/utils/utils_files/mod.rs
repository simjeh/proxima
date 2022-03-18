use std::env;
use std::fs::{File, read_dir};
use std::io::Read;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
// use termion::{style, color};
// use crate::utils::utils_console_output::{print_termion_string, PrintMode};
use crate::utils::utils_errors::OptimaError;

/// Convenience struct that holds many class functions related to file utils.
pub struct FileUtils;
impl FileUtils {
    /// Returns file path to the location from which the program is being executed.
    pub fn get_path_to_src() -> PathBuf {
        let path_buf = env::current_dir().expect("error");
        return path_buf;
    }
    /// Reads contents of file and outputs it to a string.
    pub fn read_file_contents_to_string(p: &PathBuf) -> Result<String, OptimaError> {
        let mut file_res = File::open(p);
        return match &mut file_res {
            Ok(f) => {
                let mut contents = String::new();
                f.read_to_string(&mut contents);
                Ok(contents)
            }
            Err(e) => {
                Err(OptimaError::new_generic_error_str(e.to_string().as_str()))
            }
        }
    }
    /// Returns file extension of path as string.
    pub fn get_file_extension_string(p: &PathBuf) -> Option<String> {
        let e = p.extension();
        return match e {
            None => { None }
            Some(o) => { Some(o.to_str().expect("error").to_string()) }
        }
    }
    /// Returns the paths of all files within a directory.
    pub fn get_all_files_in_directory(p: &PathBuf) -> Result<Vec<PathBuf>, OptimaError> {
        let mut out: Vec<PathBuf> = Vec::new();
        let it_res = read_dir(p.clone());
        match it_res {
            Ok(it) => {
                for i in it {
                    let path = i.expect("error").path();
                    out.push(path);
                }
            }
            Err(_) => {
                return Err(OptimaError::new_generic_error_string(format!("filepath {:?} does not exist.", p)));
            }
        }
        Ok(out)
    }
    /// Saves given object to a file as a JSON string.  The object must be serializable using serde json.
    pub fn save_object_to_file_as_json<T: Serialize>(object: &T, p: &PathBuf) -> Result<(), OptimaError> {
        let mut file_res = File::create(p);
        return match &mut file_res {
            Ok(f) => {
                serde_json::to_writer(f, object);
                Ok(())
            }
            Err(e) => {
                Err(OptimaError::new_generic_error_str(e.to_string().as_str()))
            }
        }
    }
    /// Reads object that was serialized by serde JSON from a file.
    /// ## Example
    /// ```
    /// use std::path::Path;
    /// use nalgebra::Vector3;
    /// use optima::utils::utils_files::FileUtils;
    ///
    /// let res = FileUtils::load_object_from_json_file::<Vector3<f64>>(&Path::new("data.json").to_path_buf());
    /// ```
    pub fn load_object_from_json_file<T: DeserializeOwned>(p: &PathBuf) -> Result<T, OptimaError> {
        let contents = Self::read_file_contents_to_string(p);
        return match &contents {
            Ok(s) => {
                let o: T = serde_json::from_str(s.as_str()).expect("error");
                Ok(o)
            }
            Err(e) => {
                Err(e.clone())
            }
        }
    }
}

/// Convenience struct that holds many class functions related to the assets folder utils.
pub struct AssetFolderUtils;
impl AssetFolderUtils {
    /// Returns file path to the Optima toolbox assets directory.
    /// This is read in from a file, path_to_optima_toolbox_assets.json, which is stored in the folder
    /// that the program is being executed from.
    /// If this file is not present, this function will automatically write a file for the user.
    /// If this file contains inaccurate information, this function will return an error.
    pub fn get_path_to_assets_dir() -> Result<PathBuf, OptimaError> {
        let mut path_to_assets_dir_file = FileUtils::get_path_to_src();
        path_to_assets_dir_file.push("path_to_optima_toolbox_assets.json");
        let path_exists = path_to_assets_dir_file.exists();

        return match path_exists {
            true => {
                let path_to_assets_dir_res = FileUtils::load_object_from_json_file::<PathToAssetsDir>(&path_to_assets_dir_file);
                match &path_to_assets_dir_res {
                    Ok(p) => {
                        let path_buffer = p.path_to_assets_dir.clone();
                        let path_exists = path_buffer.exists();
                        match path_exists {
                            true => {
                                Ok(path_buffer)
                            }
                            false => {
                                let console_strings = vec![
                                    format!("The path specified in path_to_optima_toolbox_assets.json file at {:?} is incorrect.", FileUtils::get_path_to_src()),
                                    format!("Please correct this path and re-run the application.")
                                ];

                                for s in &console_strings {
                                    // print_termion_string(s.as_str(),
                                    //                     PrintMode::Println,
                                    //                     color::Red,
                                    //                     true);
                                    println!("{}", s);
                                }
                                Err(OptimaError::new_generic_error_str("The path specified in path_to_optima_toolbox_assets.json is incorrect."))
                            }
                        }
                    }
                    Err(e) => {
                        let console_strings = vec![
                            format!("The path specified in path_to_optima_toolbox_assets.json file at {:?} is not a valid path.", FileUtils::get_path_to_src()),
                            format!("Please correct this path and re-run the application.")
                        ];

                        for s in &console_strings {
                            // print_termion_string(s.as_str(),
                            //                      PrintMode::Println,
                            //                      color::Red,
                            //                      true);
                            println!("{}", s);
                        }
                        Err(e.clone())
                    }
                }
            }
            false => {
                let console_strings = vec![
                    format!("I noticed that there is not a path_to_optima_toolbox_assets.json file at {:?}", FileUtils::get_path_to_src()),
                    format!("{}", "I will save a file there."),
                    format!("{}", "Please open the file at this location and fill in the absolute path.  Once this path is specified, please run the program again.")
                ];

                for s in &console_strings {
                    // print_termion_string(s.as_str(),
                    //                      PrintMode::Println,
                    //                      color::Cyan,
                    //                      true);
                    println!("{}", s);
                }

                let pp = PathToAssetsDir::default();
                FileUtils::save_object_to_file_as_json(&pp, &path_to_assets_dir_file);
                Err(OptimaError::new_generic_error_str("path_to_optima_toolbox_assets.json file did not exist yet."))
            }
        }
    }

    /// Returns file path to the given location in the Optima toolbox assets directory
    pub fn get_path_to_asset_dir_location(l: AssetFolderLocation) -> Result<PathBuf, OptimaError> {
        let mut p = Self::get_path_to_assets_dir()?;
        let a = l.get_path_wrt_asset_folder();
        p = p.join(a);
        return Ok(p);
    }
}

/// Asset folder location.  Will be used to easily access paths to these locations with respect to
/// the asset folder.
#[derive(Clone, Debug)]
pub enum AssetFolderLocation {
    Robots,
    Robot { robot_name: String },
    RobotMeshes { robot_name: String  },
    RobotPreprocessedData { robot_name: String },
    RobotConvexShapes { robot_name: String },
    RobotConvexSubcomponents { robot_name: String },
    Environments,
    FileIO
}
impl AssetFolderLocation {
    pub fn get_path_wrt_asset_folder(&self) -> PathBuf {
        return match self {
            AssetFolderLocation::Robots => {
                Path::new("optima_robots").to_path_buf()
            }
            AssetFolderLocation::Robot { robot_name } => {
                let mut out_path = Self::Robots.get_path_wrt_asset_folder();
                out_path = out_path.join(robot_name.as_str());
                out_path
            }
            AssetFolderLocation::RobotMeshes { robot_name } => {
                let mut out_path = Self::Robot { robot_name: robot_name.clone() }.get_path_wrt_asset_folder();
                out_path = out_path.join("meshes");
                out_path
            }
            AssetFolderLocation::RobotPreprocessedData { robot_name } => {
                let mut out_path = Self::Robot { robot_name: robot_name.clone() }.get_path_wrt_asset_folder();
                out_path = out_path.join("preprocessed_data");
                out_path
            }
            AssetFolderLocation::RobotConvexShapes { robot_name } => {
                let mut out_path = Self::RobotPreprocessedData { robot_name: robot_name.clone() }.get_path_wrt_asset_folder();
                out_path = out_path.join("convex_shapes");
                out_path
            }
            AssetFolderLocation::RobotConvexSubcomponents { robot_name } => {
                let mut out_path = Self::RobotPreprocessedData { robot_name: robot_name.clone() }.get_path_wrt_asset_folder();
                out_path = out_path.join("convex_shape_subcomponents");
                out_path
            }
            AssetFolderLocation::Environments => {
                Path::new("environments").to_path_buf()
            }
            AssetFolderLocation::FileIO => {
                Path::new("fileIO").to_path_buf()
            }
        }
    }
}

/// Convenience class that will be used for path_to_assets_dir.json file.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PathToAssetsDir {
    path_to_assets_dir: PathBuf
}
impl Default for PathToAssetsDir {
    fn default() -> Self {
        let mut path = FileUtils::get_path_to_src();
        path.push("..");
        path.push("optima_assets");
        Self {
            path_to_assets_dir: path
        }
    }
}

/// Convenience struct that holds many class functions related to the robot folder within assets.
pub struct RobotFolderUtils;
impl RobotFolderUtils {
    pub fn get_path_to_urdf_file(robot_name: &str) -> Result<PathBuf, OptimaError> {
        let path = AssetFolderUtils::get_path_to_asset_dir_location(AssetFolderLocation::Robot { robot_name: robot_name.to_string() })?;
        let all_files = FileUtils::get_all_files_in_directory(&path)?;
        for f in &all_files {
            let ext_option = f.extension();
            if let Some(ext) = ext_option {
                if ext == "urdf" || ext == "URDF" {
                    return Ok(f.clone());
                }
            }
        }
        return Err(OptimaError::new_generic_error_str(format!("Robot directory for robot {:?} does not contain a urdf.", robot_name).as_str()))
    }
}
