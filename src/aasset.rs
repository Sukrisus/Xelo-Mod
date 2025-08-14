use crate::ResourceLocation;
use crate::config::{is_no_hurt_cam_enabled, is_no_fog_enabled, is_java_cubemap_enabled, is_particles_disabler_enabled, is_java_clouds_enabled, is_classic_skins_enabled, is_cape_physics_enabled, is_night_vision_enabled, is_xelo_title_enabled, is_client_capes_enabled, is_block_whiteoutline_enabled};
use libc::{off64_t, off_t};
use materialbin::{CompiledMaterialDefinition, MinecraftVersion};
use ndk::asset::Asset;
use ndk_sys::{AAsset, AAssetManager};
use once_cell::sync::Lazy;
use scroll::Pread;
use serde_json::{Value, Map};
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::{CStr, CString, OsStr},
    io::{self, Cursor, Read, Seek, Write},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(PartialEq, Eq, Hash)]
struct AAssetPtr(*const ndk_sys::AAsset);
unsafe impl Send for AAssetPtr {}

static MC_VERSION: OnceLock<Option<MinecraftVersion>> = OnceLock::new();

static WANTED_ASSETS: Lazy<Mutex<HashMap<AAssetPtr, Cursor<Vec<u8>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const LEGACY_CUBEMAP_MATERIAL_BIN: &[u8] = include_bytes!("java_cubemap/LegacyCubemap.material.bin");
const RENDER_CHUNK_MATERIAL_BIN: &[u8] = include_bytes!("no_fog_materials/RenderChunk.material.bin");

const CAPE_TEXTURE_PATH: &str = "/storage/emulated/0/Android/data/com.origin.launcher/files/origin_mods/xelo_cape.png";

const TITLE_PNG: &[u8] = include_bytes!("minecraft_title_5.png");

// Cape animation JSON for cape physics
const CAPE_ANIMATION_JSON: &str = r#"{
	"format_version": "1.8.0",
	"animations": {
		"animation.player.cape": {
			"loop": true,
			"bones": {
				"cape": {
					"rotation": ["math.clamp(math.lerp(0, -110, query.cape_flap_amount) - (13 * query.modified_move_speed), -70, 0)", "query.modified_move_speed * math.pow(math.sin(query.body_y_rotation - query.head_y_rotation(0)), 3) * 55", 0],
					"position": [0, 0, "query.get_root_locator_offset('armor_offset.default_neck', 1)"]
				},
				"part1": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * (math.cos(query.modified_distance_moved * 18) * 16)", 0, "0"]
				},
				"part2": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(22 - query.modified_distance_moved * 18) * 13", 0, 0],
					"scale": 1
				},
				"part3": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(50 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part4": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(76 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part5": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(100 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part6": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(122 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part7": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(142 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part8": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(160 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part9": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(176 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part10": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(190 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part11": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(202 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part12": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(212 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part13": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(220 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part14": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(226 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part15": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(230 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"part16": {
					"rotation": ["math.clamp(query.cape_flap_amount, 0, 0.5) * math.cos(232 - query.modified_distance_moved * 18) * 13", 0, 0]
				},
				"shoulders": {
					"rotation": [0, "query.modified_move_speed * math.pow(math.sin(query.body_y_rotation - query.head_y_rotation(0)), 3) * 60", 0]
				}
			}
		}
	}
}"#;

// Cape geometry JSON for cape physics
const CAPE_GEO_JSON: &str = r#"{
	"format_version": "1.12.0",
	"minecraft:geometry": [
		{
			"description": {
				"identifier": "geometry.cape",
				"texture_width": 64,
				"texture_height": 32,
				"visible_bounds_width": 2,
				"visible_bounds_height": 3.5,
				"visible_bounds_offset": [0, 1.25, 0]
			},
			"bones": [
				{
					"name": "root",
					"pivot": [0, 0, 0]
				},
				{
					"name": "waist",
					"parent": "root",
					"pivot": [0, 12, 0]
				},
				{
					"name": "body",
					"parent": "waist",
					"pivot": [0, 24, 0]
				},
				{
					"name": "cape",
					"parent": "body",
					"pivot": [0, 24, 2],
					"rotation": [0, 180, 0]
				},
				{
					"name": "part1",
					"parent": "cape",
					"pivot": [0, 24, 2],
					"cubes": [
						{
							"origin": [-5, 23, 1],
							"size": [10, 1, 1],
							"uv": {
								"north": {"uv": [1, 1], "uv_size": [10, 1]},
								"east": {"uv": [0, 1], "uv_size": [1, 1]},
								"south": {"uv": [12, 1], "uv_size": [10, 1]},
								"west": {"uv": [11, 1], "uv_size": [1, 1]},
								"up": {"uv": [1, 1], "uv_size": [10, -1]}
							}
						}
					]
				},
				{
					"name": "part2",
					"parent": "part1",
					"pivot": [0, 23, 1],
					"cubes": [
						{
							"origin": [-5, 22, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 1.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 1.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 1.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 1.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part3",
					"parent": "part2",
					"pivot": [0, 22, 1],
					"cubes": [
						{
							"origin": [-5, 21, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 2.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 2.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 2.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 2.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part4",
					"parent": "part3",
					"pivot": [0, 21, 1],
					"cubes": [
						{
							"origin": [-5, 20, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 3.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 3.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 3.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 3.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part5",
					"parent": "part4",
					"pivot": [0, 20, 1],
					"cubes": [
						{
							"origin": [-5, 19, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 4.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 4.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 4.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 4.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part6",
					"parent": "part5",
					"pivot": [0, 19, 1],
					"cubes": [
						{
							"origin": [-5, 18, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 5.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 5.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 5.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 5.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part7",
					"parent": "part6",
					"pivot": [0, 18, 1],
					"cubes": [
						{
							"origin": [-5, 17, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 6.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 6.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 6.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 6.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part8",
					"parent": "part7",
					"pivot": [0, 17, 1],
					"cubes": [
						{
							"origin": [-5, 16, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 7.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 7.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 7.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 7.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part9",
					"parent": "part8",
					"pivot": [0, 16, 1],
					"cubes": [
						{
							"origin": [-5, 15, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 8.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 8.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 8.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 8.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part10",
					"parent": "part9",
					"pivot": [0, 15, 1],
					"cubes": [
						{
							"origin": [-5, 14, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 9.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 9.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 9.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 9.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part11",
					"parent": "part10",
					"pivot": [0, 14, 1],
					"cubes": [
						{
							"origin": [-5, 13, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 10.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 10.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 10.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 10.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part12",
					"parent": "part11",
					"pivot": [0, 13, 1],
					"cubes": [
						{
							"origin": [-5, 12, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 11.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 11.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 11.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 11.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part13",
					"parent": "part12",
					"pivot": [0, 12, 1],
					"cubes": [
						{
							"origin": [-5, 11, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 12.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 12.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 12.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 12.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part14",
					"parent": "part13",
					"pivot": [0, 11, 1],
					"cubes": [
						{
							"origin": [-5, 10, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 13.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 13.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 13.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 13.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part15",
					"parent": "part14",
					"pivot": [0, 10, 1],
					"cubes": [
						{
							"origin": [-5, 9, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 14.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 14.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 14.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 14.5], "uv_size": [1, 1.5]}
							}
						}
					]
				},
				{
					"name": "part16",
					"parent": "part15",
					"pivot": [0, 9, 1],
					"cubes": [
						{
							"origin": [-5, 8, 1],
							"size": [10, 1.5, 1],
							"uv": {
								"north": {"uv": [1, 15.5], "uv_size": [10, 1.5]},
								"east": {"uv": [0, 15.5], "uv_size": [1, 1.5]},
								"south": {"uv": [12, 15.5], "uv_size": [10, 1.5]},
								"west": {"uv": [11, 15.5], "uv_size": [1, 1.5]},
								"down": {"uv": [11, 1], "uv_size": [10, -1]}
							}
						}
					]
				}
			]
		}
	]
}"#;

const MOBS_JSON: &[u8] = include_bytes!("cape_physics/mobs.json");
// const PLAYER_ANIMATION_JSON: &[u8] = include_bytes!("cape_physics/player.animation.json"); // Removed - no longer needed

const RENDER_CHUNK_NV_MATERIAL_BIN: &[u8] = include_bytes!("nightvision_materials/RenderChunk.material.bin");

const CUSTOM_SPLASHES_JSON: &str = r#"{"splashes":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"]}"#;

const CUSTOM_FIRST_PERSON_JSON: &str = r#"{"format_version":"1.18.10","minecraft:camera_entity":{"description":{"identifier":"minecraft:first_person"},"components":{"minecraft:camera":{"field_of_view":66,"near_clipping_plane":0.025,"far_clipping_plane":2500},"minecraft:camera_first_person":{},"minecraft:camera_render_first_person_objects":{},"minecraft:camera_attach_to_player":{},"minecraft:camera_offset":{"view":[0,0],"entity":[0,0,0]},"minecraft:camera_direct_look":{"pitch_min":-89.9,"pitch_max":89.9},"minecraft:camera_perspective_option":{"view_mode":"first_person"},"minecraft:update_player_from_camera":{"look_mode":"along_camera"},"minecraft:extend_player_rendering":{},"minecraft:camera_player_sleep_vignette":{},"minecraft:vr_comfort_move":{},"minecraft:default_input_camera":{},"minecraft:gameplay_affects_fov":{},"minecraft:allow_inside_block":{}}}}"#;
const CUSTOM_THIRD_PERSON_JSON: &str = r#"{"format_version":"1.18.10","minecraft:camera_entity":{"description":{"identifier":"minecraft:third_person"},"components":{"minecraft:camera":{"field_of_view":66,"near_clipping_plane":0.025,"far_clipping_plane":2500},"minecraft:camera_third_person":{},"minecraft:camera_render_player_model":{},"minecraft:camera_attach_to_player":{},"minecraft:camera_offset":{"view":[0,0],"entity":[0,2,5]},"minecraft:camera_look_at_player":{},"minecraft:camera_orbit":{"azimuth_smoothing_spring":0,"polar_angle_smoothing_spring":0,"distance_smoothing_spring":0,"polar_angle_min":0.1,"polar_angle_max":179.9,"radius":4},"minecraft:camera_avoidance":{"relax_distance_smoothing_spring":0,"distance_constraint_min":0.25},"minecraft:camera_perspective_option":{"view_mode":"third_person"},"minecraft:update_player_from_camera":{"look_mode":"along_camera"},"minecraft:camera_player_sleep_vignette":{},"minecraft:gameplay_affects_fov":{},"minecraft:allow_inside_block":{},"minecraft:extend_player_rendering":{}}}}"#;
const CUSTOM_THIRD_PERSON_FRONT_JSON: &str = r#"{"format_version":"1.18.10","minecraft:camera_entity":{"description":{"identifier":"minecraft:third_person_front"},"components":{"minecraft:camera":{"field_of_view":66,"near_clipping_plane":0.025,"far_clipping_plane":2500},"minecraft:camera_third_person":{},"minecraft:camera_render_player_model":{},"minecraft:camera_attach_to_player":{},"minecraft:camera_offset":{"view":[0,0],"entity":[0,2,5]},"minecraft:camera_look_at_player":{},"minecraft:camera_orbit":{"azimuth_smoothing_spring":0,"polar_angle_smoothing_spring":0,"distance_smoothing_spring":0,"polar_angle_min":0.1,"polar_angle_max":179.9,"radius":4,"invert_x_input":true},"minecraft:camera_avoidance":{"relax_distance_smoothing_spring":0,"distance_constraint_min":0.25},"minecraft:camera_perspective_option":{"view_mode":"third_person_front"},"minecraft:update_player_from_camera":{"look_mode":"at_camera"},"minecraft:camera_player_sleep_vignette":{},"minecraft:gameplay_affects_fov":{},"minecraft:allow_inside_block":{},"minecraft:extend_player_rendering":{}}}}"#;

const CUSTOM_LOADING_MESSAGES_JSON: &str = r#"{"beginner_loading_messages":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"],"mid_game_loading_messages":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"],"late_game_loading_messages":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"],"creative_loading_messages":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"],"editor_loading_messages":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"],"realms_loading_messages":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"],"addons_loading_messages":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"],"store_progress_tooltips":["Xelo Client","Xelo > any other client","The Best Client!!","BlueCat","Xelo is so much better","Xelo Optimizes like no other client","Make Sure to star our repository:https://github.com/Xelo-Client/Xelo","Contributions open!","Made by the community, for the community","Yami is goated!!"]}"#;

const CUSTOM_SKINS_JSON: &str = r#"{"skins":[{"localization_name":"Steve","geometry":"geometry.humanoid.custom","texture":"steve.png","type":"free"},{"localization_name":"Alex","geometry":"geometry.humanoid.customSlim","texture":"alex.png","type":"free"}],"serialize_name":"Standard","localization_name":"Standard"}"#;

const CUSTOM_BLOCKOUTLINE: &str = r#"{"materials":{"block_overlay":{"+states":["Blending","DisableDepthWrite","DisableAlphaWrite","StencilWrite","EnableStencilTest"],"backFace":{"stencilDepthFailOp":"Keep","stencilFailOp":"Keep","stencilFunc":"NotEqual","stencilPassOp":"Replace"},"depthBias":100.0,"depthBiasOGL":100.0,"depthFunc":"LessEqual","fragmentShader":"shaders/texture_cutout.fragment","frontFace":{"stencilDepthFailOp":"Keep","stencilFailOp":"Keep","stencilFunc":"NotEqual","stencilPassOp":"Replace"},"msaaSupport":"Both","slopeScaledDepthBias":15.0,"slopeScaledDepthBiasOGL":20.0,"stencilReadMask":2,"stencilRef":2,"stencilWriteMask":2,"variants":[{"skinning":{"+defines":["USE_SKINNING"],"vertexFields":[{"field":"Position"},{"field":"BoneId0"},{"field":"UV1"},{"field":"UV0"}]}}],"vertexFields":[{"field":"Position"},{"field":"UV1"},{"field":"UV0"}],"vertexShader":"shaders/uv.vertex","vrGeometryShader":"shaders/uv.geometry"},"cracks_overlay:block_overlay":{"+samplerStates":[{"samplerIndex":0,"textureFilter":"Point"}],"blendDst":"Zero","blendSrc":"DestColor","depthFunc":"LessEqual","fragmentShader":"shaders/texture.fragment"},"cracks_overlay_alpha_test:cracks_overlay":{"+defines":["ALPHA_TEST"],"+states":["DisableCulling"]},"cracks_overlay_tile_entity:cracks_overlay":{"+samplerStates":[{"samplerIndex":0,"textureWrap":"Repeat"}],"variants":[{"skinning":{"+defines":["USE_SKINNING"],"vertexFields":[{"field":"Position"},{"field":"BoneId0"},{"field":"Normal"},{"field":"UV0"}]}}],"vertexFields":[{"field":"Position"},{"field":"Normal"},{"field":"UV0"}],"vertexShader":"shaders/uv_scale.vertex","vrGeometryShader":"shaders/uv.geometry"},"debug":{"depthFunc":"LessEqual","fragmentShader":"shaders/color.fragment","msaaSupport":"Both","vertexFields":[{"field":"Position"},{"field":"Color"}],"vertexShader":"shaders/color.vertex","vrGeometryShader":"shaders/color.geometry"},"fullscreen_cube_overlay":{"+samplerStates":[{"samplerIndex":0,"textureFilter":"Point"}],"depthFunc":"Always","fragmentShader":"shaders/texture_ccolor.fragment","msaaSupport":"Both","vertexFields":[{"field":"Position"},{"field":"UV0"}],"vertexShader":"shaders/uv.vertex","vrGeometryShader":"shaders/uv.geometry"},"fullscreen_cube_overlay_blend:fullscreen_cube_overlay":{"+states":["Blending"]},"fullscreen_cube_overlay_opaque:fullscreen_cube_overlay":{"+states":["DisableCulling"]},"lightning":{"+states":["DisableCulling","Blending"],"blendDst":"One","blendSrc":"SourceAlpha","depthFunc":"LessEqual","fragmentShader":"shaders/lightning.fragment","msaaSupport":"Both","vertexFields":[{"field":"Position"},{"field":"Color"}],"vertexShader":"shaders/color.vertex","vrGeometryShader":"shaders/color.geometry"},"name_tag":{"+samplerStates":[{"samplerIndex":0,"textureFilter":"Point"}],"+states":["Blending","DisableDepthWrite"],"depthFunc":"Always","fragmentShader":"shaders/current_color.fragment","msaaSupport":"Both","vertexFields":[{"field":"Position"}],"vertexShader":"shaders/position.vertex","vrGeometryShader":"shaders/position.geometry"},"name_tag_depth_tested:name_tag":{"depthFunc":"LessEqual"},"name_text_depth_tested:sign_text":{},"overlay_quad":{"+samplerStates":[{"samplerIndex":0,"textureFilter":"Bilinear"}],"+states":["DisableCulling","DisableDepthWrite","Blending"],"blendDst":"OneMinusSrcAlpha","blendSrc":"SourceAlpha","depthFunc":"Always","fragmentShader":"shaders/texture_raw_alphatest.fragment","vertexFields":[{"field":"Position"},{"field":"UV0"}],"vertexShader":"shaders/uv.vertex","vrGeometryShader":"shaders/uv.geometry"},"overlay_quad_clear":{"depthFunc":"Always","fragmentShader":"shaders/color.fragment","msaaSupport":"Both","vertexFields":[{"field":"Position"}],"vertexShader":"shaders/simple.vertex","vrGeometryShader":"shaders/color.geometry"},"plankton:precipitation":{"+defines":["COMFORT_MODE","FLIP_OCCLUSION","NO_VARIETY"]},"precipitation":{"+defines":["COMFORT_MODE"],"+samplerStates":[{"samplerIndex":0,"textureFilter":"Point"},{"samplerIndex":1,"textureFilter":"Point"},{"samplerIndex":2,"textureFilter":"Bilinear"}],"+states":["DisableCulling","DisableDepthWrite","Blending"],"blendDst":"OneMinusSrcAlpha","blendSrc":"SourceAlpha","depthFunc":"LessEqual","fragmentShader":"shaders/rain_snow.fragment","msaaSupport":"Both","vertexFields":[{"field":"Position"},{"field":"Color"},{"field":"UV0"}],"vertexShader":"shaders/rain_snow.vertex","vrGeometryShader":"shaders/rain_snow.geometry"},"rain:precipitation":{},"selection_box":{"+defines":["LINE_STRIP"],"depthFunc":"LessEqual","fragmentShader":"shaders/selection_box.fragment","msaaSupport":"Both","primitiveMode":"Line","vertexFields":[{"field":"Position"}],"vertexShader":"shaders/selection_box.vertex","vrGeometryShader":"shaders/position.geometry"},"selection_overlay:block_overlay":{"blendDst":"SourceColor","blendSrc":"DestColor","vertexShader":"shaders/uv_selection_overlay.vertex"},"selection_overlay_alpha:selection_overlay_level":{"vertexFields":[{"field":"Position"},{"field":"UV1"},{"field":"UV0"}]},"selection_overlay_block_entity:selection_overlay":{"variants":[{"skinning":{"+defines":["USE_SKINNING"],"vertexFields":[{"field":"Position"},{"field":"BoneId0"},{"field":"Normal"},{"field":"UV0"}]},"skinning_color":{"+defines":["USE_SKINNING"],"vertexFields":[{"field":"Position"},{"field":"BoneId0"},{"field":"Color"},{"field":"Normal"},{"field":"UV0"}]}}],"vertexFields":[{"field":"Position"},{"field":"Normal"},{"field":"UV0"}]},"selection_overlay_double_sided:selection_overlay":{"+states":["DisableCulling"]},"selection_overlay_item:selection_overlay":{},"selection_overlay_level:selection_overlay":{"msaaSupport":"Both","vertexFields":[{"field":"Position"},{"field":"Normal"},{"field":"UV0"}]},"selection_overlay_opaque:selection_overlay":{"fragmentShader":"shaders/current_color.fragment","msaaSupport":"Both","vertexShader":"shaders/selection_box.vertex","vrGeometryShader":"shaders/position.geometry"},"sign_text":{"+defines":["ALPHA_TEST","USE_LIGHTING"],"+samplerStates":[{"samplerIndex":0,"textureFilter":"Point"}],"+states":["Blending"],"depthBias":10.0,"depthBiasOGL":10.0,"depthFunc":"LessEqual","fragmentShader":"shaders/text.fragment","msaaSupport":"Both","slopeScaledDepthBias":2.0,"slopeScaledDepthBiasOGL":10.0,"vertexFields":[{"field":"Position"},{"field":"Color"},{"field":"UV0"}],"vertexShader":"shaders/color_uv.vertex","vrGeometryShader":"shaders/color_uv.geometry"},"snow:precipitation":{"+defines":["SNOW"]},"version":"1.0.0"}}"#;

// Empty JSON for disabling particles
const EMPTY_JSON: &str = "{}";

// Empty particle effect JSON that disables the particle
const EMPTY_PARTICLE_JSON: &str = r#"{"format_version":"1.10.0","particle_effect":{"description":{"identifier":"minecraft:disabled","basic_render_parameters":{"material":"particles_alpha","texture":"textures/particle/particles"}},"components":{"minecraft:emitter_lifetime_once":{"active_time":0},"minecraft:emitter_rate_instant":{"num_particles":0},"minecraft:emitter_shape_sphere":{"radius":0},"minecraft:particle_lifetime_expression":{"max_lifetime":0},"minecraft:particle_initial_speed":0,"minecraft:particle_motion_dynamic":{},"minecraft:particle_appearance_billboard":{"size":[0,0],"uv":{"texture_width":128,"texture_height":128,"uv":[0,0],"uv_size":[0,0]}}}}}"#;

// Cape render controller removed - cape physics functionality disabled
// const RENDER_JSON: &str = r#"{
//     "format_version": "1.8.0",
//     "render_controllers": {
//         "controller.render.player.cape": {
//             "rebuild_animation_matrices": true,
//             "geometry": "Geometry.cape",
//             "materials": [
//                 {
//                     "*": "Material.cape"
//                 }
//             ],
//             "textures": [
//                 "Texture.cape"
//             ]
//         }
//     }
// }"#;

const CLASSIC_STEVE_TEXTURE: &[u8] = include_bytes!("s.png");
const CLASSIC_ALEX_TEXTURE: &[u8] = include_bytes!("a.png");

const JAVA_CLOUDS_TEXTURE: &[u8] = include_bytes!("Diskksks.png");

fn get_current_mcver(man: ndk::asset::AssetManager) -> Option<MinecraftVersion> {
    let mut file = match get_uitext(man) {
        Some(asset) => asset,
        None => {
            log::error!("Shader fixing is disabled as no mc version was found");
            return None;
        }
    };
    let mut buf = Vec::with_capacity(file.length());
    if let Err(e) = file.read_to_end(&mut buf) {
        log::error!("Something is wrong with AssetManager, mc detection failed: {e}");
        return None;
    };
    for version in materialbin::ALL_VERSIONS {
        if buf
            .pread_with::<CompiledMaterialDefinition>(0, version)
            .is_ok()
        {
            log::info!("Mc version is {version}");
            return Some(version);
        };
    }
    None
}

fn get_uitext(man: ndk::asset::AssetManager) -> Option<Asset> {
    const NEW: &CStr = c"assets/renderer/materials/UIText.material.bin";
    const OLD: &CStr = c"renderer/materials/UIText.material.bin";
    for path in [NEW, OLD] {
        if let Some(asset) = man.open(path) {
            return Some(asset);
        }
    }
    None
}

macro_rules! folder_list {
    ($( apk: $apk_folder:literal -> pack: $pack_folder:expr),
        *,
    ) => {
        [
            $(($apk_folder, $pack_folder)),*,
        ]
    }
}

fn get_no_fog_material_data(filename: &str) -> Option<&'static [u8]> {
    if !is_no_fog_enabled() {
        return None;
    }
    
    match filename {
        "RenderChunk.material.bin" => Some(RENDER_CHUNK_MATERIAL_BIN),
        _ => None,
    }
}

fn get_nightvision_material_data(filename: &str) -> Option<&'static [u8]> {
    if !is_night_vision_enabled() {
        return None;
    }
    
    match filename {
        "RenderChunk.material.bin" => Some(RENDER_CHUNK_NV_MATERIAL_BIN),
        _ => None,
    }
}

fn get_java_cubemap_material_data(filename: &str) -> Option<&'static [u8]> {
    if !is_java_cubemap_enabled() {
        return None;
    }
    
    match filename {
        "LegacyCubemap.material.bin" => Some(LEGACY_CUBEMAP_MATERIAL_BIN),
        _ => None,
    }
}

fn get_title_png_data(filename: &str) -> Option<&'static [u8]> {
    if !is_xelo_title_enabled() {
        return None;
    }
    
    match filename {
        "title.png" => Some(TITLE_PNG),
        _ => None,
    }
}

fn is_particles_file_to_replace(c_path: &Path) -> bool {
    if !is_particles_disabler_enabled() {
        return false;
    }
    
    let path_str = c_path.to_string_lossy().to_lowercase();
    let filename = match c_path.file_name() {
        Some(name) => name.to_string_lossy().to_lowercase(),
        None => return false,
    };
    
    // Check if the file is in a particles or effects directory
    if (path_str.contains("/particles/") || path_str.contains("\\particles\\") ||
        path_str.contains("/effects/") || path_str.contains("\\effects\\") ||
        path_str.contains("/particle/") || path_str.contains("\\particle\\") ||
        path_str.contains("/effect/") || path_str.contains("\\effect\\")) && 
       filename.ends_with(".json") {
        return true;
    }
    
    // Check for particle-related files in any location
    if filename.ends_with(".json") && (
        filename.contains("particle") || 
        filename.contains("effect") || 
        filename.contains("explosion") || 
        filename.contains("smoke") || 
        filename.contains("flame") || 
        filename.contains("fire") || 
        filename.contains("bubble") || 
        filename.contains("dust") || 
        filename.contains("drip") || 
        filename.contains("splash") || 
        filename.contains("spell") || 
        filename.contains("portal") ||
        filename.contains("emitter") ||
        filename.contains("sparkle") ||
        filename.contains("trail") ||
        filename.contains("glow") ||
        filename.contains("steam") ||
        filename.contains("mist") ||
        filename.contains("vapor") ||
        filename.contains("ash") ||
        filename.contains("spark") ||
        filename.contains("debris")
    ) {
        return true;
    }
    
    // Specific particle files that might not match the patterns above
    let specific_particle_files = [
        "arrowspell.json",
        "balloon_gas.json",
        "basic_bubble.json",
        "basic_bubble_manual.json",
        "basic_crit.json",
        "basic_flame.json",
        "basic_portal.json",
        "basic_smoke.json",
        "bleach.json",
        "block_destruct.json",
        "breaking_item_icon.json",
        "breaking_item_terrain.json",
        "bubble_column_bubble.json",
        "bubble_column_down.json",
        "bubble_column_up.json",
        "camera_shoot_explosion.json",
        "campfire_smoke.json",
        "campfire_smoke_tall.json",
        "cauldron_bubble.json",
        "cauldron_splash.json",
        "cauldronspell.json",
        "colored_flame.json",
        "conduit.json",
        "conduit_absorb.json",
        "conduit_attack.json",
        "critical_hit.json",
        "dolphin_move.json",
        "dragon_breath_fire.json",
        "dragon_breath_lingering.json",
        "dragon_breath_trail.json",
        "dragon_death_explosion.json",
        "dragon_destroy_block.json",
        "dragon_dying_explosion.json",
        "enchanting_table_particle.json",
        "end_chest.json",
        "endrod.json",
        "evaporation_elephant_toothpaste.json",
        "evocation_fang.json",
        "evoker_spell.json",
        "explosion_cauldron.json",
        "explosion_death.json",
        "explosion_egg_destroy.json",
        "explosion_eyeofender_death.json",
        "explosion_labtable_fire.json",
        "explosion_level.json",
        "explosion_manual.json",
        "eye_of_ender_bubble.json",
        "falling_border_dust.json",
        "falling_dust.json",
        "falling_dust_concrete_powder.json",
        "falling_dust_dragon_egg.json",
        "falling_dust_gravel.json",
        "falling_dust_red_sand.json",
        "falling_dust_sand.json",
        "falling_dust_scaffolding.json",
        "falling_dust_top_snow.json",
        "fish_hook.json",
        "fish_pos.json",
        "guardian_attack.json",
        "guardian_water_move.json",
        "heart.json",
        "huge_explosion_lab_misc.json",
        "huge_explosion_level.json",
        "ice_evaporation.json",
        "ink.json",
        "knockback_roar.json",
        "lab_table_heatblock_dust.json",
        "lab_table_misc_mystical.json",
        "large_explosion_level.json",
        "lava_drip.json",
        "lava_particle.json",
        "llama_spit.json",
        "magnesium_salts.json",
        "mob_block_spawn.json",
        "mob_portal.json",
        "mobflame.json",
        "mobflame_single.json",
        "mobspell.json",
        "mycelium_dust.json",
        "note.json",
        "obsidian_glow_dust.json",
        "phantom_trail.json",
        "portal_directional.json",
        "portal_east_west.json",
        "portal_north_south.json",
        "rain_splash.json",
        "redstone_ore_dust.json",
        "redstone_repeater_dust.json",
        "redstone_torch_dust.json",
        "redstone_wire_dust.json",
        "rising_border_dust.json",
        "shulker_bullet.json",
        "silverfish_grief.json",
        "sneeze.json",
        "sparkler.json",
        "splashpotionspell.json",
        "sponge_absorb_bubble.json",
        "squid_flee.json",
        "squid_ink_bubble.json",
        "squid_move.json",
        "stunned.json",
        "totem.json",
        "totem_manual.json",
        "underwater_torch_bubble.json",
        "villager_angry.json",
        "villager_happy.json",
        "water_drip.json",
        "water_evaporation_actor.json",
        "water_evaporation_bucket.json",
        "water_evaporation_manual.json",
        "water_splash.json",
        "water_splash_manual.json",
        "water_wake.json",
        "witchspell.json",
        "wither_boss_invulnerable.json",
        // Additional common particle files
        "flame.json",
        "fire.json",
        "torch_flame.json",
        "campfire_smoke.json",
        "soul_fire_flame.json",
        "soul_torch_flame.json",
        "lava.json",
        "water.json",
        "snow.json",
        "rain.json",
        "cloud.json",
        "wind.json",
        "ambient.json",
        "environmental.json",
        "weather.json",
    ];
    
    specific_particle_files.contains(&filename.as_ref())
}

// Enhanced cape_invisible texture detection with more patterns
fn is_cape_invisible_texture_file(c_path: &Path) -> bool {
    if !is_client_capes_enabled() {
        return false;
    }
    
    let path_str = c_path.to_string_lossy();
    let filename = c_path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    
    // Check for cape_invisible texture in various possible locations
    let cape_invisible_patterns = [
        "textures/entity/cape_invisible.png",
        "/textures/entity/cape_invisible.png",
        "textures/entity/cape_invisible",
        "/textures/entity/cape_invisible",
        "entity/cape_invisible.png",
        "/entity/cape_invisible.png",
        "entity/cape_invisible",
        "/entity/cape_invisible",
        "resource_packs/vanilla/textures/entity/cape_invisible.png",
        "assets/resource_packs/vanilla/textures/entity/cape_invisible.png",
        "vanilla/textures/entity/cape_invisible.png",
        "resource_packs/vanilla/textures/entity/cape_invisible",
        "assets/resource_packs/vanilla/textures/entity/cape_invisible",
        "vanilla/textures/entity/cape_invisible",
    ];
    
    // Also check if filename itself is cape_invisible.png
    if filename == "cape_invisible.png" || filename == "cape_invisible" {
        return true;
    }
    
    cape_invisible_patterns.iter().any(|pattern| {
        path_str.contains(pattern) || path_str.ends_with(pattern)
    })
}

// Enhanced clouds detection with more patterns
fn is_clouds_texture_file(c_path: &Path) -> bool {
    if !is_java_clouds_enabled() {
        return false;
    }
    
    let path_str = c_path.to_string_lossy();
    
    let cloud_patterns = [
        "textures/environment/clouds.png",
        "/textures/environment/clouds.png",
        "environment/clouds.png",
        "/environment/clouds.png",
        "clouds.png",
        "textures/clouds.png",
        "/textures/clouds.png",
        "resource_packs/vanilla/textures/environment/clouds.png",
        "assets/resource_packs/vanilla/textures/environment/clouds.png",
        "vanilla/textures/environment/clouds.png",
    ];
    
    cloud_patterns.iter().any(|pattern| {
        path_str.contains(pattern) || path_str.ends_with(pattern)
    })
}

fn is_skin_file_path(c_path: &Path, filename: &str) -> bool {
    let path_str = c_path.to_string_lossy();
    
    let possible_paths = [
        format!("vanilla/{}", filename),
        format!("skin_packs/vanilla/{}", filename),
        format!("resource_packs/vanilla/{}", filename),
        format!("assets/skin_packs/vanilla/{}", filename),
    ];
    
    possible_paths.iter().any(|path| {
        path_str.contains(path) || path_str.ends_with(path)
    })
}

fn is_classic_skins_steve_texture_file(c_path: &Path) -> bool {
    if !is_classic_skins_enabled() {
        return false;
    }
    
    is_skin_file_path(c_path, "steve.png")
}

fn is_classic_skins_alex_texture_file(c_path: &Path) -> bool {
    if !is_classic_skins_enabled() {
        return false;
    }
    
    is_skin_file_path(c_path, "alex.png")
}

fn is_classic_skins_json_file(c_path: &Path) -> bool {
    if !is_classic_skins_enabled() {
        return false;
    }
    
    is_skin_file_path(c_path, "skins.json")
}

// Enhanced cape render controllers detection
fn is_client_capes_file(c_path: &Path) -> bool {
    if !is_client_capes_enabled() {
        return false;
    }
    
    let filename = match c_path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => return false,
    };
    
    // Check for cape render controller files
    let cape_render_files = [
        "cape.render_controllers.json"
    ];
    
    cape_render_files.contains(&filename.as_ref())
}

fn is_outline_material_file(c_path: &Path) -> bool {
    if !is_block_whiteoutline_enabled() {
        return false;
    }
    
    let filename = match c_path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => return false,
    };
    
    // Check for cape render controller files
    let outline_material_files = [
        "ui3D.material"
    ];
    
    outline_material_files.contains(&filename.as_ref())
}

fn is_persona_file_to_block(c_path: &Path) -> bool {
    if !is_classic_skins_enabled() {
        return false;
    }
    
    let path_str = c_path.to_string_lossy();
    
    let blocked_personas = [
        "persona/08_Kai_Dcast.json",
        "persona/07_Zuri_Dcast.json", 
        "persona/06_Efe_Dcast.json",
        "persona/05_Makena_Dcast.json",
        "persona/04_Sunny_Dcast.json",
        "persona/03_Ari_Dcast.json",
        "persona/02_ Noor_Dcast.json", 
    ];
    
    blocked_personas.iter().any(|persona_path| {
        path_str.contains(persona_path) || path_str.ends_with(persona_path)
    })
}

// This function is now deprecated - we'll modify the game's mobs.json directly
fn get_cape_model_data(filename: &str) -> Option<&'static [u8]> {
    // No longer serve static mobs.json - we'll intercept and modify the original instead
    None
}

// Cape physics animation data - now provides actual animation
fn get_cape_animation_data(filename: &str) -> Option<&'static [u8]> {
    if !is_cape_physics_enabled() {
        return None;
    }
    
    match filename {
        "cape.animation.json" => Some(CAPE_ANIMATION_JSON.as_bytes()),
        _ => None,
    }
}

fn get_cape_geometry_data(filename: &str) -> Option<&'static [u8]> {
    if !is_cape_physics_enabled() {
        return None;
    }
    
    match filename {
        "cape.geo.json" => Some(CAPE_GEO_JSON.as_bytes()),
        _ => None,
    }
}

fn get_particle_replacement_data(filename: &str) -> &'static str {
    // For most particle files, use empty particle JSON
    // For specific files that might need just empty JSON, use EMPTY_JSON
    match filename.to_lowercase().as_str() {
        // Some files might work better with just empty JSON
        f if f.contains("manifest") || f.contains("config") => EMPTY_JSON,
        // Default to empty particle effect JSON for particle files
        _ => EMPTY_PARTICLE_JSON,
    }
}

// Enhanced player.entity.json detection
fn is_player_entity_file(c_path: &Path) -> bool {
    if !is_client_capes_enabled() {
        return false;
    }
    
    let path_str = c_path.to_string_lossy();
    let filename = match c_path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => return false,
    };
    
    // Must be exactly player.entity.json
    if filename != "player.entity.json" {
        return false;
    }
    
    // Check if it's in a valid entity location
    let player_entity_patterns = [
        "entity/player.entity.json",
        "/entity/player.entity.json",
        "entities/player.entity.json", 
        "/entities/player.entity.json",
        "resource_packs/vanilla/entity/player.entity.json",
        "assets/resource_packs/vanilla/entity/player.entity.json",
        "vanilla/entity/player.entity.json",
        "assets/entity/player.entity.json",
        "assets/entities/player.entity.json",
    ];
    
    player_entity_patterns.iter().any(|pattern| {
        path_str.contains(pattern) || path_str.ends_with(pattern)
    })
}

// Contents.json detection for adding cape file paths
fn is_contents_json_file(c_path: &Path) -> bool {
    if !is_cape_physics_enabled() {
        return false;
    }
    
    let path_str = c_path.to_string_lossy();
    let filename = match c_path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => return false,
    };
    
    // Must be exactly contents.json
    if filename != "contents.json" {
        return false;
    }
    
    // Check if it's in a valid versioned vanilla resource pack location
    let contents_patterns = [
        "vanilla_1.19.40/contents.json",
        "/vanilla_1.19.40/contents.json",
        "resource_packs/vanilla_1.19.40/contents.json",
        "assets/resource_packs/vanilla_1.19.40/contents.json",
        "/resource_packs/vanilla_1.19.40/contents.json",
        "/assets/resource_packs/vanilla_1.19.40/contents.json",
        // Also support other version patterns
        "vanilla_1.19.41/contents.json",
        "vanilla_1.19.42/contents.json",
        "vanilla_1.19.43/contents.json",
        "vanilla_1.19.44/contents.json",
        "vanilla_1.20/contents.json",
    ];
    
    contents_patterns.iter().any(|pattern| {
        path_str.contains(pattern) || path_str.ends_with(pattern)
    })
}

// Cape animation file detection
fn is_cape_animation_file(c_path: &Path) -> bool {
    if !is_cape_physics_enabled() {
        return false;
    }
    
    let path_str = c_path.to_string_lossy();
    let filename = match c_path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => return false,
    };
    
    // Check for cape.animation.json
    if filename == "cape.animation.json" {
        // Check if it's in a valid versioned vanilla animations location
        let animation_patterns = [
            "vanilla_1.19.40/animations/cape.animation.json",
            "/vanilla_1.19.40/animations/cape.animation.json",
            "resource_packs/vanilla_1.19.40/animations/cape.animation.json",
            "assets/resource_packs/vanilla_1.19.40/animations/cape.animation.json",
            "/resource_packs/vanilla_1.19.40/animations/cape.animation.json",
            "/assets/resource_packs/vanilla_1.19.40/animations/cape.animation.json",
            // Support other versions
            "vanilla_1.19.41/animations/cape.animation.json",
            "vanilla_1.19.42/animations/cape.animation.json",
            "vanilla_1.19.43/animations/cape.animation.json",
            "vanilla_1.19.44/animations/cape.animation.json",
            "vanilla_1.20/animations/cape.animation.json",
        ];
        
        return animation_patterns.iter().any(|pattern| {
            path_str.contains(pattern) || path_str.ends_with(pattern)
        });
    }
    
    false
}

// Cape geometry file detection
fn is_cape_geometry_file(c_path: &Path) -> bool {
    if !is_cape_physics_enabled() {
        return false;
    }
    
    let path_str = c_path.to_string_lossy();
    let filename = match c_path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => return false,
    };
    
    // Check for cape.geo.json
    if filename == "cape.geo.json" {
        // Check if it's in a valid versioned vanilla models location
        let geometry_patterns = [
            "vanilla_1.19.40/models/entity/cape.geo.json",
            "/vanilla_1.19.40/models/entity/cape.geo.json",
            "resource_packs/vanilla_1.19.40/models/entity/cape.geo.json",
            "assets/resource_packs/vanilla_1.19.40/models/entity/cape.geo.json",
            "/resource_packs/vanilla_1.19.40/models/entity/cape.geo.json",
            "/assets/resource_packs/vanilla_1.19.40/models/entity/cape.geo.json",
            // Support other versions
            "vanilla_1.19.41/models/entity/cape.geo.json",
            "vanilla_1.19.42/models/entity/cape.geo.json",
            "vanilla_1.19.43/models/entity/cape.geo.json",
            "vanilla_1.19.44/models/entity/cape.geo.json",
            "vanilla_1.20/models/entity/cape.geo.json",
        ];
        
        return geometry_patterns.iter().any(|pattern| {
            path_str.contains(pattern) || path_str.ends_with(pattern)
        });
    }
    
    false
}

// Removed game's mobs.json detection - no longer needed with versioned vanilla approach

// Improved custom cape texture loading with better error handling
fn load_custom_cape_texture() -> Option<Vec<u8>> {
    match std::fs::read(CAPE_TEXTURE_PATH) {
        Ok(data) => {
            if data.is_empty() {
                log::warn!("Custom cape texture file is empty: {}", CAPE_TEXTURE_PATH);
                return None;
            }
            log::info!("Successfully loaded custom cape texture from: {} ({} bytes)", CAPE_TEXTURE_PATH, data.len());
            Some(data)
        }
        Err(e) => {
            log::warn!("Failed to load custom cape texture from {}: {}", CAPE_TEXTURE_PATH, e);
            log::info!("Make sure xelo_cape.png exists in the origin_mods folder and is a valid PNG file");
            None
        }
    }
}

// Improved player.entity.json modification with better error handling
fn modify_player_entity_json(original_data: &[u8]) -> Option<Vec<u8>> {
    let json_str = match std::str::from_utf8(original_data) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to parse player.entity.json as UTF-8: {}", e);
            return None;
        }
    };
    
    let mut json_value: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to parse player.entity.json as JSON: {}", e);
            return None;
        }
    };
    
    // Navigate to the render_controllers array
    if let Some(client_entity) = json_value
        .get_mut("minecraft:client_entity")
        .and_then(|ce| ce.as_object_mut())
    {
        if let Some(description) = client_entity
            .get_mut("description")
            .and_then(|desc| desc.as_object_mut())
        {
            // Get the existing render_controllers array
            if let Some(render_controllers) = description
                .get_mut("render_controllers")
                .and_then(|rc| rc.as_array_mut())
            {
                // Create the cape render controller object
                let cape_controller = serde_json::json!({
                    "controller.render.player.cape": "(query.armor_texture_slot(1) != 5) && (!variable.is_first_person || variable.is_paperdoll) && (!variable.map_face_icon)"
                });
                
                // Check if cape controller already exists
                let cape_exists = render_controllers.iter().any(|controller| {
                    if let Some(obj) = controller.as_object() {
                        obj.contains_key("controller.render.player.cape")
                    } else {
                        false
                    }
                });
                
                if !cape_exists {
                    render_controllers.push(cape_controller);
                    log::info!("Added cape render controller to player.entity.json");
                } else {
                    log::info!("Cape render controller already exists in player.entity.json");
                }
            } else {
                log::error!("render_controllers array not found in player.entity.json");
                return None;
            }
            
            // Verify textures section has cape texture (should already exist in the default file)
            if let Some(textures) = description.get("textures").and_then(|t| t.as_object()) {
                if textures.contains_key("cape") {
                    log::info!("Cape texture reference already exists in player.entity.json");
                } else {
                    log::warn!("Cape texture reference missing from player.entity.json");
                }
            } else {
                log::error!("Textures section not found in player.entity.json");
                return None;
            }
            
        } else {
            log::error!("description object not found in player.entity.json");
            return None;
        }
    } else {
        log::error!("minecraft:client_entity not found in player.entity.json");
        return None;  
    }
    
    // Convert back to JSON string with proper formatting
    match serde_json::to_string_pretty(&json_value) {
        Ok(modified_json) => Some(modified_json.into_bytes()),
        Err(e) => {
            log::error!("Failed to serialize modified player.entity.json: {}", e);
            None
        }
    }
}

// Modify contents.json to include cape file paths
fn modify_contents_json(original_data: &[u8]) -> Option<Vec<u8>> {
    let json_str = match std::str::from_utf8(original_data) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to parse contents.json as UTF-8: {}", e);
            return None;
        }
    };
    
    let mut json_value: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to parse contents.json as JSON: {}", e);
            return None;
        }
    };
    
    // Navigate to the content array
    if let Some(content_array) = json_value
        .get_mut("content")
        .and_then(|content| content.as_array_mut())
    {
        // Define the cape file paths to add (relative to versioned vanilla folder)
        let cape_paths = [
            "models/entity/cape.geo.json",
            "animations/cape.animation.json"
        ];
        
        for cape_path in cape_paths.iter() {
            // Check if this path already exists
            let path_exists = content_array.iter().any(|item| {
                if let Some(obj) = item.as_object() {
                    if let Some(path_value) = obj.get("path") {
                        if let Some(path_str) = path_value.as_str() {
                            return path_str == *cape_path;
                        }
                    }
                }
                false
            });
            
            // Add the path if it doesn't exist
            if !path_exists {
                let cape_entry = serde_json::json!({
                    "path": cape_path
                });
                content_array.push(cape_entry);
                log::info!("Added {} to contents.json", cape_path);
            } else {
                log::debug!("Cape path {} already exists in contents.json", cape_path);
            }
        }
    } else {
        log::error!("Failed to find 'content' array in contents.json");
        return None;
    }
    
    // Convert back to JSON string with proper formatting
    match serde_json::to_string_pretty(&json_value) {
        Ok(modified_json) => {
            log::info!("Successfully modified contents.json with cape paths");
            Some(modified_json.into_bytes())
        }
        Err(e) => {
            log::error!("Failed to serialize modified contents.json: {}", e);
            None
        }
    }
}

// Removed game's mobs.json modification - no longer needed with versioned vanilla approach

pub(crate) unsafe fn open(
    man: *mut AAssetManager,
    fname: *const libc::c_char,
    mode: libc::c_int,
) -> *mut ndk_sys::AAsset {
    let aasset = unsafe { ndk_sys::AAssetManager_open(man, fname, mode) };
    let c_str = unsafe { CStr::from_ptr(fname) };
    let raw_cstr = c_str.to_bytes();
    let os_str = OsStr::from_bytes(raw_cstr);
    let c_path: &Path = Path::new(os_str);
    
    let Some(os_filename) = c_path.file_name() else {
        log::warn!("Path had no filename: {c_path:?}");
        return aasset;
    };

    // Debug logging for client capes
    if is_client_capes_enabled() {
        let path_str = c_path.to_string_lossy();
        if path_str.contains("cape") || path_str.contains("player.entity") {
            log::info!("Client capes enabled - checking file: {}", c_path.display());
        }
    }
    
    // Debug logging for particles disabler
    if is_particles_disabler_enabled() {
        let path_str = c_path.to_string_lossy();
        if path_str.contains("particle") || path_str.contains("effect") {
            log::info!("Particles disabler enabled - checking file: {}", c_path.display());
        }
    }
    
    // Handle cape_invisible texture replacement
    if is_cape_invisible_texture_file(c_path) {
        log::info!("Intercepting cape_invisible texture with custom cape: {}", c_path.display());
        
        if let Some(custom_cape_data) = load_custom_cape_texture() {
            let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
            wanted_lock.insert(AAssetPtr(aasset), Cursor::new(custom_cape_data));
            return aasset;
        } else {
            log::warn!("Custom cape texture not found, blocking cape_invisible texture");
            // Block the original cape_invisible texture if custom one isn't available
            if !aasset.is_null() {
                ndk_sys::AAsset_close(aasset);
            }
            return std::ptr::null_mut();
        }
    }

    // Block persona files if classic skins enabled
    if is_persona_file_to_block(c_path) {
        log::info!("Blocking persona file due to classic_skins enabled: {}", c_path.display());
        if !aasset.is_null() {
            ndk_sys::AAsset_close(aasset);
        }
        return std::ptr::null_mut();
    }

    // Replace particles files with empty JSON if particles disabler enabled
    if is_particles_file_to_replace(c_path) {
        let filename = c_path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        
        log::info!("Replacing particles file with empty JSON due to particles_disabler enabled: {}", c_path.display());
        let replacement_data = get_particle_replacement_data(&filename);
        let buffer = replacement_data.as_bytes().to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    // Handle player.entity.json modification
    if is_player_entity_file(c_path) {
        log::info!("Intercepting player.entity.json with client capes modification: {}", c_path.display());
        
        // Read the original file first
        if aasset.is_null() {
            log::error!("Failed to open original player.entity.json");
            return aasset;
        }
        
        let length = ndk_sys::AAsset_getLength(aasset) as usize;
        if length == 0 {
            log::error!("player.entity.json has zero length");
            return aasset;
        }
        
        let mut original_data = vec![0u8; length];
        let bytes_read = ndk_sys::AAsset_read(aasset, original_data.as_mut_ptr() as *mut libc::c_void, length);
        
        if bytes_read != length as i32 {
            log::error!("Failed to read original player.entity.json completely (read {}, expected {})", bytes_read, length);
            return aasset;
        }
        
        // Reset the asset position for normal operation
        ndk_sys::AAsset_seek(aasset, 0, libc::SEEK_SET);
        
        if let Some(modified_data) = modify_player_entity_json(&original_data) {
            let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
            wanted_lock.insert(AAssetPtr(aasset), Cursor::new(modified_data));
            return aasset;
        } else {
            log::warn!("Failed to modify player.entity.json, using original");
            return aasset;
        }
    }
    
    // Handle contents.json modification to add cape file paths
    if is_contents_json_file(c_path) {
        log::info!("Intercepting contents.json to add cape file paths: {}", c_path.display());
        
        // Read the original file first
        if aasset.is_null() {
            log::error!("Failed to open original contents.json");
            return aasset;
        }
        
        let length = ndk_sys::AAsset_getLength(aasset) as usize;
        if length == 0 {
            log::error!("contents.json has zero length");
            return aasset;
        }
        
        let mut original_data = vec![0u8; length];
        let bytes_read = ndk_sys::AAsset_read(aasset, original_data.as_mut_ptr() as *mut libc::c_void, length);
        
        if bytes_read != length as i32 {
            log::error!("Failed to read original contents.json completely (read {}, expected {})", bytes_read, length);
            return aasset;
        }
        
        // Reset the asset position for normal operation
        ndk_sys::AAsset_seek(aasset, 0, libc::SEEK_SET);
        
        if let Some(modified_data) = modify_contents_json(&original_data) {
            let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
            wanted_lock.insert(AAssetPtr(aasset), Cursor::new(modified_data));
            return aasset;
        } else {
            log::warn!("Failed to modify contents.json, using original");
            return aasset;
        }
    }
    
    // Handle cape animation file requests
    if is_cape_animation_file(c_path) {
        log::info!("Serving cape animation file: {}", c_path.display());
        let buffer = CAPE_ANIMATION_JSON.as_bytes().to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    // Handle cape geometry file requests
    if is_cape_geometry_file(c_path) {
        log::info!("Serving cape geometry file: {}", c_path.display());
        let buffer = CAPE_GEO_JSON.as_bytes().to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    // Removed complex mobs.json modification - now using versioned vanilla folder approach
    
    // Custom splashes
    if os_filename == "splashes.json" {
        log::info!("Intercepting splashes.json with custom content");
        let buffer = CUSTOM_SPLASHES_JSON.as_bytes().to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    // Custom loading messages
    if os_filename == "loading_messages.json" {
        log::info!("Intercepting loading_messages.json with custom content");
        let buffer = CUSTOM_LOADING_MESSAGES_JSON.as_bytes().to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    // Java clouds texture replacement
    if is_clouds_texture_file(c_path) {
        log::info!("Intercepting clouds texture with Java clouds texture: {}", c_path.display());
        let buffer = JAVA_CLOUDS_TEXTURE.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }

    // Classic skins replacements
    if is_classic_skins_steve_texture_file(c_path) {
        log::info!("Intercepting steve.png with classic Steve texture: {}", c_path.display());
        let buffer = CLASSIC_STEVE_TEXTURE.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    if is_classic_skins_alex_texture_file(c_path) {
        log::info!("Intercepting alex.png with classic Alex texture: {}", c_path.display());
        let buffer = CLASSIC_ALEX_TEXTURE.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    if is_classic_skins_json_file(c_path) {
        log::info!("Intercepting skins.json with classic skins content: {}", c_path.display());
        let buffer = CUSTOM_SKINS_JSON.as_bytes().to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    // Cape render controllers disabled - cape physics functionality removed
    // if is_client_capes_file(c_path) {
    //     log::info!("Intercepting cape render controller file with cape content: {}", c_path.display());
    //     let buffer = RENDER_JSON.as_bytes().to_vec();
    //     let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
    //     wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
    //     return aasset;
    // }
    
    if is_outline_material_file(c_path) {
        log::info!("Intercepting  ui3dmaterial file with new content: {}", c_path.display());
        let buffer = CUSTOM_BLOCKOUTLINE.as_bytes().to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    // No hurt cam camera replacements
    if is_no_hurt_cam_enabled() {
        let path_str = c_path.to_string_lossy();
        
        if path_str.contains("cameras/") {
            if os_filename == "first_person.json" {
                log::info!("Intercepting cameras/first_person.json with custom content (nohurtcam enabled)");
                let buffer = CUSTOM_FIRST_PERSON_JSON.as_bytes().to_vec();
                let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
                wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
                return aasset;
            }
            
            if os_filename == "third_person.json" {
                log::info!("Intercepting cameras/third_person.json with custom content (nohurtcam enabled)");
                let buffer = CUSTOM_THIRD_PERSON_JSON.as_bytes().to_vec();
                let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
                wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
                return aasset;
            }
            
            if os_filename == "third_person_front.json" {
                log::info!("Intercepting cameras/third_person_front.json with custom content (nohurtcam enabled)");
                let buffer = CUSTOM_THIRD_PERSON_FRONT_JSON.as_bytes().to_vec();
                let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
                wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
                return aasset;
            }
        }
    }

    // Material replacements
    let filename_str = os_filename.to_string_lossy();
    if let Some(no_fog_data) = get_no_fog_material_data(&filename_str) {
        log::info!("Intercepting {} with no-fog material (no-fog enabled)", filename_str);
        let buffer = no_fog_data.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    if let Some(night_vision_data) = get_nightvision_material_data(&filename_str) {
        log::info!("Intercepting {} with night-vision material (night-vision enabled)", filename_str);
        let buffer = night_vision_data.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    // Legacy cape physics file handling - now handled by specific file detection above
    // These functions are deprecated but kept for compatibility
    if let Some(cape_physics_animation_data) = get_cape_animation_data(&filename_str) {
        log::info!("Intercepting {} with cape-physics animation (cape-physics enabled)", filename_str);
        let buffer = cape_physics_animation_data.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    if let Some(cape_physics_model_data) = get_cape_model_data(&filename_str) {
        log::info!("Intercepting {} with cape-physics model (cape-physics enabled)", filename_str);
        let buffer = cape_physics_model_data.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    if let Some(java_cubemap_data) = get_java_cubemap_material_data(&filename_str) {
        log::info!("Intercepting {} with java-cubemap material (java-cubemap enabled)", filename_str);
        let buffer = java_cubemap_data.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }
    
    if let Some(title_png_data) = get_title_png_data(&filename_str) {
        log::info!("Intercepting {} with xelo title png (xelo-title enabled)", filename_str);
        let buffer = title_png_data.to_vec();
        let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
        wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
        return aasset;
    }

    // Resource pack loading logic
    let stripped = match c_path.strip_prefix("assets/") {
        Ok(yay) => yay,
        Err(_e) => c_path,
    };
    
    let replacement_list = folder_list! {
        apk: "gui/dist/hbui/" -> pack: "hbui/",
        apk: "skin_packs/persona/" -> pack: "persona/",
        apk: "renderer/" -> pack: "renderer/",
        apk: "resource_packs/vanilla/cameras/" -> pack: "vanilla_cameras/",
    };
    
    for replacement in replacement_list {
        if let Ok(file) = stripped.strip_prefix(replacement.0) {
            cxx::let_cxx_string!(cxx_out = "");
            let loadfn = match crate::RPM_LOAD.get() {
                Some(ptr) => ptr,
                None => {
                    log::warn!("ResourcePackManager fn is not ready yet?");
                    return aasset;
                }
            };
            let mut arraybuf = [0; 128];
            let file_path = opt_path_join(&mut arraybuf, &[Path::new(replacement.1), file]);
            let packm_ptr = crate::PACKM_OBJ.load(std::sync::atomic::Ordering::Acquire);
            let resource_loc = ResourceLocation::from_str(file_path.as_ref());
            log::info!("loading rpck file: {:#?}", &file_path);
            if packm_ptr.is_null() {
                log::error!("ResourcePackManager ptr is null");
                return aasset;
            }
            loadfn(packm_ptr, resource_loc, cxx_out.as_mut());
            if cxx_out.is_empty() {
                log::info!("File was not found");
                return aasset;
            }
            let buffer = if os_filename.as_encoded_bytes().ends_with(b".material.bin") {
                match process_material(man, cxx_out.as_bytes()) {
                    Some(updated) => updated,
                    None => cxx_out.as_bytes().to_vec(),
                }
            } else {
                cxx_out.as_bytes().to_vec()
            };
            let mut wanted_lock = WANTED_ASSETS.lock().unwrap();
            wanted_lock.insert(AAssetPtr(aasset), Cursor::new(buffer));
            return aasset;
        }
    }
    return aasset;
}

fn opt_path_join<'a>(bytes: &'a mut [u8; 128], paths: &[&Path]) -> Cow<'a, CStr> {
    let total_len: usize = paths.iter().map(|p| p.as_os_str().len()).sum();
    if total_len + 1 > 128 {
        let mut pathbuf = PathBuf::new();
        for path in paths {
            pathbuf.push(path);
        }
        let cpath = CString::new(pathbuf.into_os_string().as_encoded_bytes()).unwrap();
        return Cow::Owned(cpath);
    }

    let mut writer = bytes.as_mut_slice();
    for path in paths {
        let osstr = path.as_os_str().as_bytes();
        let _ = writer.write(osstr);
    }
    let _ = writer.write(&[0]);
    let guh = CStr::from_bytes_until_nul(bytes).unwrap();
    Cow::Borrowed(guh)
}

fn process_material(man: *mut AAssetManager, data: &[u8]) -> Option<Vec<u8>> {
    let mcver = MC_VERSION.get_or_init(|| {
        let pointer = match std::ptr::NonNull::new(man) {
            Some(yay) => yay,
            None => {
                log::warn!("AssetManager is null?, preposterous, mc detection failed");
                return None;
            }
        };
        let manager = unsafe { ndk::asset::AssetManager::from_ptr(pointer) };
        get_current_mcver(manager)
    });
    let mcver = (*mcver)?;
    for version in materialbin::ALL_VERSIONS {
        let material: CompiledMaterialDefinition = match data.pread_with(0, version) {
            Ok(data) => data,
            Err(e) => {
                log::trace!("[version] Parsing failed: {e}");
                continue;
            }
        };
        if version == mcver {
            return None;
        }
        let mut output = Vec::with_capacity(data.len());
        if let Err(e) = material.write(&mut output, mcver) {
            log::trace!("[version] Write error: {e}");
            return None;
        }
        return Some(output);
    }

    None
}

pub(crate) unsafe fn seek64(aasset: *mut AAsset, off: off64_t, whence: libc::c_int) -> off64_t {
    let mut wanted_assets = WANTED_ASSETS.lock().unwrap();
    let file = match wanted_assets.get_mut(&AAssetPtr(aasset)) {
        Some(file) => file,
        None => return ndk_sys::AAsset_seek64(aasset, off, whence),
    };
    seek_facade(off, whence, file) as off64_t
}

pub(crate) unsafe fn seek(aasset: *mut AAsset, off: off_t, whence: libc::c_int) -> off_t {
    let mut wanted_assets = WANTED_ASSETS.lock().unwrap();
    let file = match wanted_assets.get_mut(&AAssetPtr(aasset)) {
        Some(file) => file,
        None => return ndk_sys::AAsset_seek(aasset, off, whence),
    };
    seek_facade(off.into(), whence, file) as off_t
}

pub(crate) unsafe fn read(
    aasset: *mut AAsset,
    buf: *mut libc::c_void,
    count: libc::size_t,
) -> libc::c_int {
    let mut wanted_assets = WANTED_ASSETS.lock().unwrap();
    let file = match wanted_assets.get_mut(&AAssetPtr(aasset)) {
        Some(file) => file,
        None => return ndk_sys::AAsset_read(aasset, buf, count),
    };
    let rs_buffer = core::slice::from_raw_parts_mut(buf as *mut u8, count);
    let read_total = match file.read(rs_buffer) {
        Ok(n) => n,
        Err(e) => {
            log::warn!("failed fake aaset read: {e}");
            return -1 as libc::c_int;
        }
    };
    read_total as libc::c_int
}

pub(crate) unsafe fn len(aasset: *mut AAsset) -> off_t {
    let wanted_assets = WANTED_ASSETS.lock().unwrap();
    let file = match wanted_assets.get(&AAssetPtr(aasset)) {
        Some(file) => file,
        None => return ndk_sys::AAsset_getLength(aasset),
    };
    file.get_ref().len() as off_t
}

pub(crate) unsafe fn len64(aasset: *mut AAsset) -> off64_t {
    let wanted_assets = WANTED_ASSETS.lock().unwrap();
    let file = match wanted_assets.get(&AAssetPtr(aasset)) {
        Some(file) => file,
        None => return ndk_sys::AAsset_getLength64(aasset),
    };
    file.get_ref().len() as off64_t
}

pub(crate) unsafe fn rem(aasset: *mut AAsset) -> off_t {
    let wanted_assets = WANTED_ASSETS.lock().unwrap();
    let file = match wanted_assets.get(&AAssetPtr(aasset)) {
        Some(file) => file,
        None => return ndk_sys::AAsset_getRemainingLength(aasset),
    };
    (file.get_ref().len() - file.position() as usize) as off_t
}

pub(crate) unsafe fn rem64(aasset: *mut AAsset) -> off64_t {
    let wanted_assets = WANTED_ASSETS.lock().unwrap();
    let file = match wanted_assets.get(&AAssetPtr(aasset)) {
        Some(file) => file,
        None => return ndk_sys::AAsset_getRemainingLength64(aasset),
    };
    (file.get_ref().len() - file.position() as usize) as off64_t
}

pub(crate) unsafe fn close(aasset: *mut AAsset) {
    let mut wanted_assets = WANTED_ASSETS.lock().unwrap();
    if wanted_assets.remove(&AAssetPtr(aasset)).is_none() {
        ndk_sys::AAsset_close(aasset);
    }
}

pub(crate) unsafe fn get_buffer(aasset: *mut AAsset) -> *const libc::c_void {
    let mut wanted_assets = WANTED_ASSETS.lock().unwrap();
    let file = match wanted_assets.get_mut(&AAssetPtr(aasset)) {
        Some(file) => file,
        None => return ndk_sys::AAsset_getBuffer(aasset),
    };
    file.get_mut().as_mut_ptr().cast()
}

pub(crate) unsafe fn fd_dummy(
    aasset: *mut AAsset,
    out_start: *mut off_t,
    out_len: *mut off_t,
) -> libc::c_int {
    let wanted_assets = WANTED_ASSETS.lock().unwrap();
    match wanted_assets.get(&AAssetPtr(aasset)) {
        Some(_) => {
            log::error!("WE GOT BUSTED NOOO");
            -1
        }
        None => ndk_sys::AAsset_openFileDescriptor(aasset, out_start, out_len),
    }
}

pub(crate) unsafe fn fd_dummy64(
    aasset: *mut AAsset,
    out_start: *mut off64_t,
    out_len: *mut off64_t,
) -> libc::c_int {
    let wanted_assets = WANTED_ASSETS.lock().unwrap();
    match wanted_assets.get(&AAssetPtr(aasset)) {
        Some(_) => {
            log::error!("WE GOT BUSTED NOOO");
            -1
        }
        None => ndk_sys::AAsset_openFileDescriptor64(aasset, out_start, out_len),
    }
}

pub(crate) unsafe fn is_alloc(aasset: *mut AAsset) -> libc::c_int {
    let wanted_assets = WANTED_ASSETS.lock().unwrap();
    match wanted_assets.get(&AAssetPtr(aasset)) {
        Some(_) => false as libc::c_int,
        None => ndk_sys::AAsset_isAllocated(aasset),
    }
}

fn seek_facade(offset: i64, whence: libc::c_int, file: &mut Cursor<Vec<u8>>) -> i64 {
    let offset = match whence {
        libc::SEEK_SET => {
            let u64_off = match u64::try_from(offset) {
                Ok(uoff) => uoff,
                Err(e) => {
                    log::error!("signed ({offset}) to unsigned failed: {e}");
                    return -1;
                }
            };
            io::SeekFrom::Start(u64_off)
        }
        libc::SEEK_CUR => io::SeekFrom::Current(offset),
        libc::SEEK_END => io::SeekFrom::End(offset),
        _ => {
            log::error!("Invalid seek whence");
            return -1;
        }
    };
    match file.seek(offset) {
        Ok(new_offset) => match new_offset.try_into() {
            Ok(int) => int,
            Err(err) => {
                log::error!("u64 ({new_offset}) to i64 failed: {err}");
                -1
            }
        },
        Err(err) => {
            log::error!("aasset seek failed: {err}");
            -1
        }
    }
}