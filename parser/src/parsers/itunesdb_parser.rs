// src/parsers/itunesdb_parser.rs
use std::fmt::Write;
use std::fs::File;
use std::io;

use crate::constants::itunesdb_constants;
use crate::itunesdb;

use crate::helpers::helpers;
use crate::helpers::itunesdb_helpers;

use crate::itunesdb::{IpodDeviceInfo, IpodModel};

pub fn extract_device_info(itunesdb_file_as_bytes: &[u8]) -> IpodDeviceInfo {
    let mut device_info = IpodDeviceInfo::default();
    
    // Get database version
    let db_version = helpers::get_slice_as_le_u32(
        0,
        itunesdb_file_as_bytes, 
        itunesdb_constants::DATABASE_OBJECT_VERSION_NUMBER_OFFSET,
        itunesdb_constants::DATABASE_OBJECT_VERSION_NUMBER_LEN
    );

    // Get hardware capabilities/features to help identify model
    let has_artwork = check_for_artwork_support(itunesdb_file_as_bytes);
    let has_photos = check_for_photo_support(itunesdb_file_as_bytes);
    let device_capacity = estimate_device_capacity(itunesdb_file_as_bytes);
    
    // Determine model, generation, name and release year based on database version and capabilities
    let (model, generation, name, release_year) = match db_version {
        // iTunes 4.2
        0x09 => {
            if device_capacity <= 4 {
                (IpodModel::Mini("1st Generation".to_string()), 
                 "1st Generation".to_string(),
                 format!("iPod Mini {}GB (1st Gen)", device_capacity),
                 Some(2004))
            } else {
                (IpodModel::Classic("3rd Generation".to_string()),
                 "3rd Generation".to_string(),
                 format!("iPod Classic {}GB (3rd Gen)", device_capacity), 
                 Some(2003))
            }
        },

        // iTunes 4.5-4.8
        0x0A..=0x0C => {
            if device_capacity <= 6 {
                (IpodModel::Mini("2nd Generation".to_string()),
                 "2nd Generation".to_string(),
                 format!("iPod Mini {}GB (2nd Gen)", device_capacity),
                 Some(2005))
            } else {
                (IpodModel::Classic("4th Generation".to_string()),
                 "4th Generation".to_string(),
                 format!("iPod Classic {}GB (4th Gen)", device_capacity),
                 Some(2004))
            }
        },

        // iTunes 4.9-5.0
        0x0D..=0x0E => {
            if has_photos && device_capacity <= 4 {
                (IpodModel::Nano("1st Generation".to_string()),
                 "1st Generation".to_string(),
                 format!("iPod Nano {}GB (1st Gen)", device_capacity),
                 Some(2005))
            } else if has_photos {
                (IpodModel::Classic("5th Generation (Video)".to_string()),
                 "5th Generation".to_string(),
                 format!("iPod Classic {}GB Video (5th Gen)", device_capacity),
                 Some(2005))
            } else {
                (IpodModel::Classic("Color/Photo".to_string()),
                 "4th Generation".to_string(),
                 format!("iPod Classic {}GB Color/Photo (4th Gen)", device_capacity),
                 Some(2004))
            }
        },

        // iTunes 6.0-6.0.5
        0x0F..=0x12 => {
            if device_capacity <= 8 {
                (IpodModel::Nano("2nd Generation".to_string()),
                 "2nd Generation".to_string(),
                 format!("iPod Nano {}GB (2nd Gen)", device_capacity),
                 Some(2006))
            } else {
                (IpodModel::Classic("5th Generation Enhanced".to_string()),
                 "5th Generation".to_string(),
                 format!("iPod Classic {}GB Enhanced (5th Gen)", device_capacity),
                 Some(2006))
            }
        },

        // iTunes 7.0-7.2
        0x13..=0x15 => {
            if device_capacity <= 8 {
                (IpodModel::Nano("3rd Generation".to_string()),
                 "3rd Generation".to_string(),
                 format!("iPod Nano {}GB (3rd Gen)", device_capacity),
                 Some(2007))
            } else {
                (IpodModel::Classic("6th Generation".to_string()),
                 "6th Generation".to_string(),
                 format!("iPod Classic {}GB (6th Gen)", device_capacity),
                 Some(2007))
            }
        },

        // iTunes 7.3-7.4
        0x17..=0x19 => {
            if device_capacity <= 16 {
                (IpodModel::Nano("4th Generation".to_string()),
                 "4th Generation".to_string(),
                 format!("iPod Nano {}GB (4th Gen)", device_capacity),
                 Some(2008))
            } else {
                (IpodModel::Classic("6th Generation (Late 2008)".to_string()),
                 "6th Generation".to_string(),
                 format!("iPod Classic {}GB (Late 2008)", device_capacity),
                 Some(2008))
            }
        },

        _ => (IpodModel::Unknown,
              "Unknown".to_string(),
              format!("Unknown iPod ({}GB)", device_capacity),
              None)
    };
    
    device_info.model = model;
    device_info.generation = generation;
    device_info.name = name;
    device_info.release_year = release_year;

    device_info
}

// Helper functions to check device capabilities
fn check_for_artwork_support(itunesdb_file_as_bytes: &[u8]) -> bool {
    // Check for artwork flag in mhit entries
    // Look through file for track artwork flags
    let mut idx = 0;
    while idx < itunesdb_file_as_bytes.len() - itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE {
        if &itunesdb_file_as_bytes[idx..idx + itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE] == itunesdb_constants::TRACK_ITEM_KEY.as_bytes() {
            // Check artwork flag
            let artwork_flag = &itunesdb_file_as_bytes[
                idx + itunesdb_constants::TRACK_ITEM_TRACK_HAS_ARTWORK_SETTING_OFFSET..
                idx + itunesdb_constants::TRACK_ITEM_TRACK_HAS_ARTWORK_SETTING_OFFSET + 
                itunesdb_constants::TRACK_ITEM_TRACK_HAS_ARTWORK_SETTING_LEN
            ];
            if artwork_flag[0] == 0x01 {
                return true;
            }
        }
        idx += 1;
    }
    false
}

fn check_for_photo_support(itunesdb_file_as_bytes: &[u8]) -> bool {
    // Look for photo database structures
    let mut idx = 0;
    while idx < itunesdb_file_as_bytes.len() - itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE {
        // Look for mhsd type 3 which indicates photo support
        if &itunesdb_file_as_bytes[idx..idx + itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE] == itunesdb_constants::DATASET_KEY.as_bytes() {
            let dataset_type = helpers::get_slice_as_le_u32(
                idx,
                itunesdb_file_as_bytes,
                itunesdb_constants::DATASET_TYPE_OFFSET,
                itunesdb_constants::DATASET_TYPE_LEN
            );
            if dataset_type == 3 {
                return true;
            }
        }
        idx += 1;
    }
    false
}

fn estimate_device_capacity(itunesdb_file_as_bytes: &[u8]) -> u32 {
    // Estimate capacity by looking at total size of all tracks
    let mut total_size: u64 = 0;
    let mut idx = 0;
    
    while idx < itunesdb_file_as_bytes.len() - itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE {
        if &itunesdb_file_as_bytes[idx..idx + itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE] == itunesdb_constants::TRACK_ITEM_KEY.as_bytes() {
            let track_size = helpers::get_slice_as_le_u32(
                idx,
                itunesdb_file_as_bytes,
                itunesdb_constants::TRACK_ITEM_TRACK_FILE_SIZE_BYTES_OFFSET,
                itunesdb_constants::TRACK_ITEM_TRACK_FILE_SIZE_BYTES_LEN
            );
            total_size += track_size as u64;
        }
        idx += 1;
    }
    
    // Convert total bytes to GB and round up
    // Using 1GB = 1,000,000,000 bytes (marketing calculation)
    let gb = (total_size as f64 / 1_000_000_000.0).ceil() as u32;
    
    // Return estimated capacity based on total content size
    // Typically devices were sold in 2/4/8/16/32/64/128 GB capacities
    match gb {
        0..=2 => 2,
        3..=4 => 4,
        5..=8 => 8,
        9..=16 => 16,
        17..=32 => 32,
        33..=64 => 64,
        _ => 128
    }
}


pub fn parse_itunesdb_file(itunesdb_file_as_bytes : Vec<u8>) {
    let device_info = extract_device_info(&itunesdb_file_as_bytes);

    let mut music_csv_writer = helpers::init_csv_writer("music.csv");
    let mut podcast_csv_writer = helpers::init_csv_writer("podcasts.csv");

    let mut songs_found: Vec<itunesdb::Song> = Vec::new();
    let mut podcasts_found: Vec<itunesdb::Podcast> = Vec::new();

    let mut curr_song = itunesdb::Song::default();
    let mut curr_podcast = itunesdb::Podcast::default();

    // Set device info on initial objects
    curr_song.ipod_model = device_info.model.clone();
    curr_song.ipod_generation = device_info.generation.clone();
    curr_song.ipod_name = device_info.name.clone();
    curr_song.ipod_release_year = device_info.release_year;

    curr_podcast.ipod_model = device_info.model.clone();
    curr_podcast.ipod_generation = device_info.generation.clone(); 
    curr_podcast.ipod_name = device_info.name.clone();
    curr_podcast.ipod_release_year = device_info.release_year;

    let mut curr_media_type = itunesdb::HandleableMediaType::UNKNOWN;

    let mut idx = 0;

    while idx < (itunesdb_file_as_bytes.len() - itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE) {
        let potential_section_heading = &itunesdb_file_as_bytes[idx..idx + itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE];

        // Parse Database Object
        if potential_section_heading == itunesdb_constants::DATABASE_OBJECT_KEY.as_bytes() {
            let db_language_raw = helpers::get_slice_from_offset_with_len(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::DATABASE_OBJECT_LANGUAGE_OFFSET,
                itunesdb_constants::DATABASE_OBJECT_LANGUAGE_LEN,
            );

            let db_language = std::str::from_utf8(&db_language_raw)
                .expect("Can't parse database language string");

            println!(
                "File is using language: {}, and has iTunes version: {}",
                db_language,
                itunesdb::parse_version_number(helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::DATABASE_OBJECT_VERSION_NUMBER_OFFSET,
                    itunesdb_constants::DATABASE_OBJECT_VERSION_NUMBER_LEN
                ))
            );

            idx += itunesdb_constants::DATABASE_OBJECT_LAST_OFFSET;
        }
        // Parse DataSet
        else if potential_section_heading == itunesdb_constants::DATASET_KEY.as_bytes() {

            let dataset_type_raw = helpers::get_slice_from_offset_with_len(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::DATASET_TYPE_OFFSET,
                itunesdb_constants::DATASET_TYPE_LEN,
            );

            let dataset_type_parsed = itunesdb::parse_dataset_type(dataset_type_raw[0] as u32);

            // println!(
            //     "Dataset Type: {}",
            //     dataset_type_parsed
            // );

            idx += itunesdb_constants::DATASET_LAST_OFFSET;
        }
        // Parse TrackList
        else if potential_section_heading == itunesdb_constants::TRACKLIST_KEY.as_bytes() {
            let num_songs_in_db = helpers::get_slice_as_le_u32(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::TRACKLIST_NUM_SONGS_OFFSET,
                itunesdb_constants::TRACKLIST_NUM_SONGS_LEN,
            );

            println!("{} songs in tracklist", num_songs_in_db);

            idx += itunesdb_constants::TRACKLIST_LAST_OFFSET;
        } else if potential_section_heading == itunesdb_constants::TRACK_ITEM_KEY.as_bytes() {
            let mut track_item_info: String = String::new();

            write!(
                track_item_info,
                "========== Track #{} of {} ",
                helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_NUMBER_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_NUMBER_LEN
                ),
                helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_NUM_TRACKS_IN_ALBUM_OFFSET,
                    itunesdb_constants::TRACK_ITEM_NUM_TRACKS_IN_ALBUM_LEN
                )
            )
            .unwrap();

            let num_discs = helpers::get_slice_as_le_u32(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::TRACK_ITEM_TRACK_TOTAL_NUM_DISCS_OFFSET,
                itunesdb_constants::TRACK_ITEM_TRACK_TOTAL_NUM_DISCS_LEN,
            );

            // Only print disc info if current song is part of multi-disc set
            if num_discs > 0 {
                let tracks_current_disc_num = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_DISC_NUMBER_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_DISC_NUMBER_LEN,
                );

                write!(
                    track_item_info,
                    " | 💿 #{} of {}",
                    tracks_current_disc_num, num_discs
                )
                .unwrap();
            }

            write!(track_item_info, "==========\n").unwrap();

            let track_filetype_raw = &itunesdb_file_as_bytes[idx
                + itunesdb_constants::TRACK_ITEM_TRACK_FILETYPE_OFFSET
                ..idx
                    + itunesdb_constants::TRACK_ITEM_TRACK_FILETYPE_OFFSET
                    + itunesdb_constants::TRACK_ITEM_TRACK_FILETYPE_LEN];

            // TODO: encapsulate this logic elsewhere
            if helpers::build_le_u32_from_bytes(track_filetype_raw) == 0 {
                println!("Track Item file type missing. Is this is a 1st - 4th gen iPod?");
            } else {
                let track_item_extension =
                    itunesdb::decode_track_item_filetype(track_filetype_raw);
                write!(
                    track_item_info,
                    "Track extension: '{}' | ",
                    track_item_extension
                )
                .unwrap();

                curr_song.file_extension = track_item_extension;
            }

            let track_media_type_raw = &itunesdb_file_as_bytes[idx
                + itunesdb_constants::TRACK_ITEM_TRACK_MEDIA_TYPE_OFFSET
                ..idx
                    + itunesdb_constants::TRACK_ITEM_TRACK_MEDIA_TYPE_OFFSET
                    + itunesdb_constants::TRACK_ITEM_TRACK_MEDIA_TYPE_LEN];

            let track_movie_file_flag = helpers::get_slice_as_le_u32(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::TRACK_ITEM_TRACK_MOVIE_FLAG_SETTING_OFFSET,
                itunesdb_constants::TRACK_ITEM_TRACK_MOVIE_FLAG_SETTING_LEN,
            );

            let (track_media_type_name, track_media_type_enum) =
                itunesdb::decode_track_media_type(track_media_type_raw);

            write!(
                track_item_info,
                "Movie file flag: {} | Media Type: {} \n",
                (track_movie_file_flag == 1),
                track_media_type_name
            )
            .unwrap();

            if matches!(
                track_media_type_enum,
                itunesdb::HandleableMediaType::Television
            ) {
                let season_number = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_SEASON_NUMBER_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_SEASON_NUMBER_LEN,
                );

                let episode_number = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_EPISODE_NUMBER_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_EPISODE_NUMBER_LEN,
                );

                write!(
                    track_item_info,
                    "Season #{} Episode #{}",
                    season_number, episode_number
                )
                .unwrap();
            } else if matches!(
                track_media_type_enum,
                itunesdb::HandleableMediaType::SongLike
            ) {

                curr_media_type = track_media_type_enum;

                let track_advanced_audio_type = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_ADVANCED_TRACK_TYPE_OFFSET,
                    itunesdb_constants::TRACK_ITEM_ADVANCED_TRACK_TYPE_LEN,
                );

                write!(
                    track_item_info,
                    "Experimental(!) advanced audio info: {} \n",
                    itunesdb::decode_track_audio_type(track_advanced_audio_type)
                )
                .unwrap();

                let apple_user_id = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_USER_ID_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_USER_ID_LEN,
                );

                if apple_user_id != 0 {
                    write!(track_item_info, "Apple User ID: {} \n", apple_user_id).unwrap();
                }

                let track_bitrate_type_raw = &itunesdb_file_as_bytes[idx
                    + itunesdb_constants::TRACK_ITEM_TRACK_BITRATE_SETTING_OFFSET
                    ..idx
                        + itunesdb_constants::TRACK_ITEM_TRACK_BITRATE_SETTING_OFFSET
                        + itunesdb_constants::TRACK_ITEM_TRACK_BITRATE_SETTING_LEN];

                let track_bitrate = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_BITRATE_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_BITRATE_LEN,
                );

                let track_sample_rate_raw = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_SAMPLE_RATE_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_SAMPLE_RATE_LEN,
                );

                let track_sample_rate_hz =
                    itunesdb::decode_track_samplerate_to_hz(track_sample_rate_raw);

                let track_volume_setting = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_VOLUME_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_VOLUME_LEN,
                );

                let track_bpm = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_BPM_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_BPM_LEN,
                );

                write!(
                    track_item_info,
                    "[Audio info] {} kbps ({}) ~ {} Hz | {} bpm |  🔈 adj. {} \n",
                    track_bitrate,
                    itunesdb::decode_track_bitrate_type_setting(track_bitrate_type_raw),
                    track_sample_rate_hz,
                    track_bpm,
                    track_volume_setting
                )
                .unwrap();

                curr_song.bitrate_kbps = track_bitrate;
                curr_song.sample_rate_hz = track_sample_rate_hz;


                let track_size_bytes = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_FILE_SIZE_BYTES_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_FILE_SIZE_BYTES_LEN,
                );

                if track_size_bytes < 1 {
                    panic!("Error: Track must have non-zero file size");
                }

                write!(track_item_info, "Track size: {} bytes | ", track_size_bytes).unwrap();

                curr_song.set_song_filesize(track_size_bytes);

                let track_length_raw = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_LENGTH_MILLISECONDS_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_LENGTH_MILLISECONDS_LEN,
                );

                let track_length_s = itunesdb::decode_raw_track_length_to_s(track_length_raw);

                //println!("Raw track length (ms): {} | in seconds: {}", track_length_raw, track_length_s);

                curr_song.set_song_duration(track_length_raw);

                write!(
                    track_item_info,
                    "Track duration: {} (Raw = {} seconds)",
                    helpers::convert_seconds_to_human_readable_duration(track_length_s),
                    track_length_s
                )
                .unwrap();

                let track_start_time_offset = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_START_TIME_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_START_TIME_LEN,
                );

                let track_stop_time_offset = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_STOP_TIME_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_STOP_TIME_LEN,
                );

                write!(
                    track_item_info,
                    "{} \n",
                    itunesdb::get_track_length_info(
                        track_length_raw,
                        track_start_time_offset,
                        track_stop_time_offset
                    )
                )
                .unwrap();

                let track_play_count = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_PLAY_COUNT_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_PLAY_COUNT_LEN,
                );

                curr_song.num_plays = track_play_count;

                let track_skipped_count = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_SKIPPED_COUNT_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_SKIPPED_COUNT_LEN,
                );

                // TODO: WHy are the last played timestamps zero sometimes?

                let track_last_played_timestamp = helpers::get_slice_as_mac_timestamp(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_LAST_PLAYED_TIMESTAMP_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_LAST_PLAYED_TIMESTAMP_LEN,
                );

                let track_last_skipped_timestamp = helpers::get_slice_as_mac_timestamp(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_LAST_SKIPPED_TIMESTAMP_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_LAST_SKIPPED_TIMESTAMP_LEN,
                );

                let track_skip_when_shuffle_setting = &itunesdb_file_as_bytes[idx + itunesdb_constants::TRACK_ITEM_TRACK_SKIP_WHEN_SHUFFLING_SETTING_OFFSET .. idx + itunesdb_constants::TRACK_ITEM_TRACK_SKIP_WHEN_SHUFFLING_SETTING_OFFSET + itunesdb_constants::TRACK_ITEM_TRACK_SKIP_WHEN_SHUFFLING_SETTING_LEN];

                write!(track_item_info, "Play/Skip statistics: # of plays: {} , Last played on: {} | # of skips: {}, Last skipped on: {} (Skip when shuffling? {}) ", track_play_count, track_last_played_timestamp, track_skipped_count, track_last_skipped_timestamp, track_skip_when_shuffle_setting[0] ).unwrap();

                let track_is_compilation_setting_raw = &itunesdb_file_as_bytes[idx
                    + itunesdb_constants::TRACK_ITEM_IS_COMPILATION_SETTING_OFFSET
                    ..idx
                        + itunesdb_constants::TRACK_ITEM_IS_COMPILATION_SETTING_OFFSET
                        + itunesdb_constants::TRACK_ITEM_IS_COMPILATION_SETTING_LEN];

                let track_has_lyrics_setting_raw = &itunesdb_file_as_bytes[idx
                    + itunesdb_constants::TRACK_ITEM_TRACK_LYRICS_AVAILABLE_SETTING_OFFSET
                    ..idx
                        + itunesdb_constants::TRACK_ITEM_TRACK_LYRICS_AVAILABLE_SETTING_OFFSET
                        + itunesdb_constants::TRACK_ITEM_TRACK_LYRICS_AVAILABLE_SETTING_LEN];

                write!(
                    track_item_info,
                    " \n Is part of compilation? {} , Has lyrics? {}",
                    track_is_compilation_setting_raw[0], track_has_lyrics_setting_raw[0]
                )
                .unwrap();

                let track_rating = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_RATING_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_RATING_LEN,
                );

                if track_rating > 0 {
                    curr_song.song_rating_raw = track_rating as u8;

                    let track_prev_rating = helpers::get_slice_as_le_u32(
                        idx,
                        &itunesdb_file_as_bytes,
                        itunesdb_constants::TRACK_ITEM_TRACK_PREVIOUS_RATING_OFFSET,
                        itunesdb_constants::TRACK_ITEM_TRACK_PREVIOUS_RATING_LEN,
                    );

                    write!(
                        track_item_info,
                        "\n Rating info: Current rating: {} | Previous rating: {} \n",
                        itunesdb_helpers::decode_itunes_stars(track_rating as u8),
                        itunesdb_helpers::decode_itunes_stars(track_prev_rating as u8)
                    )
                    .unwrap();
                }

                let gapless_playback_setting_for_track = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_GAPLESS_PLAYBACK_SETTING_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_GAPLESS_PLAYBACK_SETTING_LEN,
                );

                if gapless_playback_setting_for_track == 1 {
                    let num_beginning_silence_samples = helpers::get_slice_as_le_u32(idx, &itunesdb_file_as_bytes, itunesdb_constants::TRACK_ITEM_TRACK_BEGINNING_SILENCE_SAMPLE_COUNT_OFFSET, itunesdb_constants::TRACK_ITEM_TRACK_BEGINNING_SILENCE_SAMPLE_COUNT_LEN);

                    let num_ending_silence_samples = helpers::get_slice_as_le_u32(
                        idx,
                        &itunesdb_file_as_bytes,
                        itunesdb_constants::TRACK_ITEM_TRACK_ENDING_SILENCE_SAMPLE_COUNT_OFFSET,
                        itunesdb_constants::TRACK_ITEM_TRACK_ENDING_SILENCE_SAMPLE_COUNT_LEN,
                    );

                    // let num_total_samples = helpers::get_slice_as_le_u32(idx, &itunesdb_file_as_bytes, iTunesDB::TRACK_ITEM_TRACK_NUM_SAMPLES_OFFSET, iTunesDB::TRACK_ITEM_TRACK_NUM_SAMPLES_LEN);

                    let num_total_samples = helpers::get_slice_as_le_u64(
                        idx,
                        &itunesdb_file_as_bytes,
                        itunesdb_constants::TRACK_ITEM_TRACK_NUM_SAMPLES_OFFSET,
                        itunesdb_constants::TRACK_ITEM_TRACK_NUM_SAMPLES_LEN,
                    );

                    write!(track_item_info, "[Gapless playback info] # of silent samples ({} at start, {} at end) - Total {}\n", num_beginning_silence_samples, num_ending_silence_samples, num_total_samples).unwrap();
                }

                let track_crossfade_setting = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_CROSSFADING_SETTING_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_CROSSFADING_SETTING_LEN,
                );

                write!(
                    track_item_info,
                    "Crossfade: {} | ",
                    (if track_crossfade_setting == 1 {
                        "Enabled"
                    } else {
                        "Disabled"
                    })
                )
                .unwrap();

                let track_has_artwork_setting = &itunesdb_file_as_bytes[idx
                    + itunesdb_constants::TRACK_ITEM_TRACK_HAS_ARTWORK_SETTING_OFFSET
                    ..idx
                        + itunesdb_constants::TRACK_ITEM_TRACK_HAS_ARTWORK_SETTING_OFFSET
                        + itunesdb_constants::TRACK_ITEM_TRACK_HAS_ARTWORK_SETTING_LEN];

                // TODO: Encapsulate this logic elsewhere
                if itunesdb::track_has_artwork(track_has_artwork_setting) {
                    let track_associated_artwork_size = helpers::get_slice_as_le_u32(
                        idx,
                        &itunesdb_file_as_bytes,
                        itunesdb_constants::TRACK_ITEM_TRACK_ARTWORK_SIZE_BYTES_OFFSET,
                        itunesdb_constants::TRACK_ITEM_TRACK_ARTWORK_SIZE_BYTES_LEN,
                    );

                    write!(
                        track_item_info,
                        "🎨 artwork size: {} bytes \n",
                        track_associated_artwork_size
                    )
                    .unwrap();
                }

                let track_year_released = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_YEAR_PUBLISHED_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_YEAR_PUBLISHED_LEN,
                );

                write!(track_item_info, "\n 🗓️  ").unwrap();

                if track_year_released != 0 {
                    write!(
                        track_item_info,
                        "Track year (from title): {} ",
                        track_year_released
                    )
                    .unwrap();

                    curr_song.song_year = track_year_released as u16;
                }

                // let track_added_timestamp = helpers::get_slice_as_mac_timestamp(
                //     idx,
                //     &itunesdb_file_as_bytes,
                //     itunesdb_constants::TRACK_ITEM_TRACK_ADDED_TIMESTAMP_OFFSET,
                //     itunesdb_constants::TRACK_ITEM_TRACK_ADDED_TIMESTAMP_LEN,
                // );

                let track_added_epoch = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_ADDED_TIMESTAMP_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_ADDED_TIMESTAMP_LEN,
                );

                if track_added_epoch > 0 {
                    let track_added_timestamp =
                        helpers::get_timestamp_as_mac(track_added_epoch as u64);

                    curr_song.set_song_added_timestamp(track_added_epoch as u64);

                    write!(
                        track_item_info,
                        "Added to library on: {} ",
                        track_added_timestamp
                    )
                    .unwrap();
                }

                let track_modified_timestamp = helpers::get_slice_as_mac_timestamp(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::TRACK_ITEM_TRACK_MODIFIED_TIME_OFFSET,
                    itunesdb_constants::TRACK_ITEM_TRACK_MODIFIED_TIME_LEN,
                );

                let track_published_to_store_timestamp: chrono::DateTime<chrono::Utc> =
                    helpers::get_slice_as_mac_timestamp(
                        idx,
                        &itunesdb_file_as_bytes,
                        itunesdb_constants::TRACK_ITEM_TRACK_RELEASED_TIMESTAMP_OFFSET,
                        itunesdb_constants::TRACK_ITEM_TRACK_RELEASED_TIMESTAMP_LEN,
                    );

                write!(
                    track_item_info,
                    "Last modified: {} Published to iTunes store: {}",
                    track_modified_timestamp, track_published_to_store_timestamp
                )
                .unwrap();

                //println!("{} \n", track_item_info);
            }

            else if matches!(
                track_media_type_enum,
                itunesdb::HandleableMediaType::Podcast) {

                    println!("TrackItem: Podcast found");

                    curr_media_type = track_media_type_enum;

                }

            idx += itunesdb_constants::TRACK_ITEM_LAST_OFFSET;
        } else if potential_section_heading == itunesdb_constants::PLAYLIST_KEY.as_bytes() {
            let mut playlist_info: String = "==== ".to_string();

            let is_master_playlist_setting = &itunesdb_file_as_bytes[idx
                + itunesdb_constants::PLAYLIST_IS_MASTER_PLAYLIST_SETTING_OFFSET
                ..idx
                    + itunesdb_constants::PLAYLIST_IS_MASTER_PLAYLIST_SETTING_OFFSET
                    + itunesdb_constants::PLAYLIST_IS_MASTER_PLAYLIST_SETTING_LEN];

            if is_master_playlist_setting[0] == 1 {
                write!(playlist_info, "Master ").unwrap();
            }

            write!(playlist_info, "Playlist found!").unwrap();

            let playlist_created_timestamp = helpers::get_slice_as_mac_timestamp(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::PLAYLIST_CREATED_TIMESTAMP_OFFSET,
                itunesdb_constants::PLAYLIST_CREATED_TIMESTAMP_LEN,
            );

            write!(
                playlist_info,
                " | Playlist created at: {} ",
                playlist_created_timestamp
            )
            .unwrap();

            let playlist_sort_order = helpers::get_slice_as_le_u32(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::PLAYLIST_PLAYLIST_SORT_ORDER_OFFSET,
                itunesdb_constants::PLAYLIST_PLAYLIST_SORT_ORDER_LEN,
            );

            write!(
                playlist_info,
                "| {} \n",
                itunesdb::decode_playlist_sort_order(playlist_sort_order)
            )
            .unwrap();

            //println!("{} ====", playlist_info);

            idx += itunesdb_constants::PLAYLIST_LAST_OFFSET;
        } else if potential_section_heading == itunesdb_constants::PLAYLIST_ITEM_KEY.as_bytes()
        {
            let mut playlist_item_info: String = "-----".to_string();

            let playlist_item_added_timestamp = helpers::get_slice_as_mac_timestamp(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::PLAYLIST_ITEM_ADDED_TIMESTAMP_OFFSET,
                itunesdb_constants::PLAYLIST_ITEM_ADDED_TIMESTAMP_LEN,
            );

            write!(
                playlist_item_info,
                " | Date added to playlist: {}",
                playlist_item_added_timestamp
            )
            .unwrap();

            //println!("{}  -----\n", playlist_item_info);

            idx += itunesdb_constants::PLAYLIST_ITEM_LAST_OFFSET;
        } else if potential_section_heading == itunesdb_constants::ALBUM_LIST_KEY.as_bytes() {
            let mut album_list_info: String = "~~~~~~~".to_string();

            let album_item_total_num_songs = helpers::get_slice_as_le_u32(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::ALBUM_LIST_TOTAL_NUM_SONGS_OFFSET,
                itunesdb_constants::ALBUM_LIST_TOTAL_NUM_SONGS_LEN,
            );

            write!(
                album_list_info,
                " {} songs in Album List",
                album_item_total_num_songs
            )
            .unwrap();

            //println!("{}  ~~~~~~~\n", album_list_info);

            idx += itunesdb_constants::ALBUM_LIST_LAST_OFFSET;
        }
        // else if potential_section_heading == iTunesDB::ALBUM_ITEM_KEY.as_bytes() {

        //     let album_item_info : String = "######## Album item found! | ".to_string();

        //     // write!(album_item_info, " {} ########\n", itunesdb_helpers::get_timestamp_as_mac(helpers::build_le_u32_from_bytes(album_item_unknown_timestamp_raw) as u64)).unwrap();

        //     println!("{} ########\n", album_item_info);

        //     idx += iTunesDB::ALBUM_ITEM_LAST_OFFSET;

        // }
        else if potential_section_heading == itunesdb_constants::DATA_OBJECT_KEY.as_bytes() {
            let mut data_object_info: String = "%%%%%%% Data Object found!\n".to_string();

            let data_object_type_raw = helpers::get_slice_as_le_u32(
                idx,
                &itunesdb_file_as_bytes,
                itunesdb_constants::DATA_OBJECT_TYPE_OFFSET,
                itunesdb_constants::DATA_OBJECT_TYPE_LEN,
            );

            write!(
                data_object_info,
                "Type (raw) = {}, Decoded= '{}' | ",
                data_object_type_raw,
                itunesdb::decode_data_object_type(data_object_type_raw)
            )
            .unwrap();

            if itunesdb::is_data_object_type_string(data_object_type_raw) {
                let data_object_string_len = helpers::get_slice_as_le_u32(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::DATA_OBJECT_STRING_LENGTH_OFFSET,
                    itunesdb_constants::DATA_OBJECT_STRING_LENGTH_LEN,
                );

                let data_object_str_bytes = helpers::get_slice_from_offset_with_len(
                    idx,
                    &itunesdb_file_as_bytes,
                    itunesdb_constants::DATA_OBJECT_STRING_LOCATION_OFFSET,
                    data_object_string_len as usize,
                );

                // let data_object_str = std::str::from_utf8(&data_object_str_bytes).expect("Can't parse string data object!");
                let data_object_str = String::from_utf16(&helpers::return_utf16_from_utf8(
                    &data_object_str_bytes,
                ))
                .expect("Can't decode string to UTF-16");

                write!(
                    data_object_info,
                    "Length= {} | Value: '{}'",
                    data_object_string_len, data_object_str
                )
                .unwrap();

                // We've found a title, now, use the TrackItem info to determine if the title is for a song or for a podcast
                if data_object_type_raw == itunesdb::HandleableDataObjectType::Title as u32
                {
                    if curr_media_type == itunesdb::HandleableMediaType::SongLike {
                        curr_song.song_title = data_object_str;
                    } 
                    else if curr_media_type == itunesdb::HandleableMediaType::Podcast {
                        curr_podcast.podcast_title = data_object_str;
                    }
                }
                 else if data_object_type_raw
                    == itunesdb::HandleableDataObjectType::Album as u32
                {
                    curr_song.song_album = data_object_str;

                } else if data_object_type_raw
                    == itunesdb::HandleableDataObjectType::Artist as u32
                {
                    if curr_media_type == itunesdb::HandleableMediaType::SongLike {
                        curr_song.song_artist = data_object_str;
                    }
                    else if curr_media_type == itunesdb::HandleableMediaType::Podcast {
                        curr_podcast.podcast_publisher = data_object_str;
                    }

                } else if data_object_type_raw
                    == itunesdb::HandleableDataObjectType::Genre as u32
                {
                    if curr_media_type == itunesdb::HandleableMediaType::SongLike {
                        curr_song.song_genre = data_object_str;
                    }

                    else if curr_media_type == itunesdb::HandleableMediaType::Podcast {
                        if curr_podcast.podcast_genre.is_empty() {
                            curr_podcast.podcast_genre = data_object_str;
                        }
                    }

                } else if data_object_type_raw
                    == itunesdb::HandleableDataObjectType::Comment as u32
                {
                    if curr_media_type == itunesdb::HandleableMediaType::SongLike {

                        curr_song.song_comment = data_object_str;

                    } else if curr_media_type == itunesdb::HandleableMediaType::Podcast {
                        curr_podcast.podcast_subtitle = data_object_str;
                    }

                } else if data_object_type_raw
                    == itunesdb::HandleableDataObjectType::Composer as u32
                {
                    curr_song.song_composer = data_object_str;

                } else if data_object_type_raw
                    == itunesdb::HandleableDataObjectType::FileLocation as u32
                {
                    curr_song.set_song_filename(data_object_str);

                    if curr_song.are_enough_fields_valid() {
                        songs_found.push(curr_song);
                        curr_song = itunesdb::Song::default();
                        // Set device info on new song
                        curr_song.ipod_model = device_info.model.clone();
                        curr_song.ipod_generation = device_info.generation.clone();
                        curr_song.ipod_name = device_info.name.clone();
                        curr_song.ipod_release_year = device_info.release_year;

                    }
                }
                else if data_object_type_raw == itunesdb::HandleableDataObjectType::FileType as u32 {
                    
                    if curr_media_type == itunesdb::HandleableMediaType::Podcast {

                        curr_podcast.podcast_file_type = data_object_str;
                    }

                }
                else if data_object_type_raw == itunesdb::HandleableDataObjectType::PodcastDescription as u32 {
                    
                    if curr_media_type == itunesdb::HandleableMediaType::Podcast {

                        curr_podcast.podcast_description = data_object_str;
                    }

                    if !curr_podcast.podcast_title.is_empty() {

                        podcasts_found.push(curr_podcast);
                        curr_podcast = itunesdb::Podcast::default();
                        // Set device info on new podcast
                        curr_podcast.ipod_model = device_info.model.clone();
                        curr_podcast.ipod_generation = device_info.generation.clone();
                        curr_podcast.ipod_name = device_info.name.clone();
                        curr_podcast.ipod_release_year = device_info.release_year; 
                    }
                }
            }
            // Non-string MHODs
            else {
                if (data_object_type_raw
                    == itunesdb::HandleableDataObjectType::PodcastEnclosureURL as u32)
                    || (data_object_type_raw
                        == itunesdb::HandleableDataObjectType::Podcast_RSS_URL as u32)
                {
                    let podcast_url = itunesdb::decode_podcast_urls(idx, &itunesdb_file_as_bytes);

                    write!(
                        data_object_info,
                        "Podcast discovered, with URL: {}",
                        podcast_url
                    )
                    .unwrap();
                }
            }

            //println!("{} %%%%%%% \r\n", data_object_info);

            idx += itunesdb_constants::DATA_OBJECT_LAST_OFFSET;
        }

        idx += itunesdb_constants::DEFAULT_SUBSTRUCTURE_SIZE;
    }

    println!("{} podcasts found", podcasts_found.len());

    println!("{} songs found", songs_found.len());

    // Add JSON output @joshkenney
    if !songs_found.is_empty() {
        // Make sure Song struct has #[derive(Serialize)] attribute
        let songs_json = serde_json::to_string_pretty(&songs_found)
            .expect("Error serializing songs to JSON");
        
        let mut songs_json_file = File::create("songs.json")
            .expect("Error creating songs JSON file");
        
        io::Write::write_all(&mut songs_json_file, songs_json.as_bytes())
            .expect("Error writing songs JSON file");
                
        println!("Created songs.json with {} songs", songs_found.len());
    }
    
    if !podcasts_found.is_empty() {
        // Make sure Podcast struct has #[derive(Serialize)] attribute
        let podcasts_json = serde_json::to_string_pretty(&podcasts_found)
            .expect("Error serializing podcasts to JSON");
                
        let mut podcasts_json_file = File::create("podcasts.json")
            .expect("Error creating podcasts JSON file");
        
        io::Write::write_all(&mut podcasts_json_file, podcasts_json.as_bytes())
            .expect("Error writing podcasts JSON file");
                
        println!("Created podcasts.json with {} podcasts", podcasts_found.len());
    }

    podcast_csv_writer.write_record(&[
        "Episode Title",
        "Publisher",
        "Genre",
        "Subtitle",
        "Description",
        "File Type",
        // Device info
        "iPod Model",
        "iPod Generation",
        "iPod Name",
        "iPod Release Year",
    ]).expect("Error can't create CSV file headers for podcast file");

    for episode in podcasts_found.iter() {
        podcast_csv_writer.write_record(&[
            episode.podcast_title.to_string(),
            episode.podcast_publisher.to_string(),
            episode.podcast_genre.to_string(),
            episode.podcast_subtitle.to_string(),
            episode.podcast_description.to_string().replace("\n", ""),
            episode.podcast_file_type.to_string(),
            // Device info
            episode.ipod_model.to_string(),
            episode.ipod_generation.to_string(),
            episode.ipod_name.to_string(),
            episode.ipod_release_year.map_or("Unknown".to_string(), |y| y.to_string()),
        ]).expect("Can't write row to podcast CSV file");
    }

    music_csv_writer
        .write_record(&[
            "Song Title",
            "Artist",
            "Album",
            "Year released",
            "File size",
            "Song Duration",
            "Filename",
            "Genre",
            "File extension",
            "Bitrate (kbps)",
            "Sample Rate (Hz)",
            "File size (bytes)",
            "Song duration (seconds)",
            "Play count",
            "Rating",
            "Added to library on (timestamp)",
            "Added to library on (epoch)",
            "Composer",
            "Comment",
            // Device info
            "iPod Model",
            "iPod Generation", 
            "iPod Name",
            "iPod Release Year",
        ])
        .expect("Can't create CSV file headers for music file");

    for song in songs_found.iter() {
        // the duplicate `to_string()` calls are to avoid this error:
        // cannot move out of `song.song_title` which is behind a shared reference
        // move occurs because `song.song_title` has type `String`, which does not implement the `Copy` trait

        music_csv_writer
            .write_record(&[
                song.song_title.to_string(),
                song.song_artist.to_string(),
                song.song_album.to_string(),
                song.song_year.to_string(),
                song.file_size_friendly.to_string(),
                song.song_duration_friendly.to_string(),
                song.song_filename.to_string(),
                song.song_genre.to_string(),
                song.file_extension.to_string(),
                song.bitrate_kbps.to_string(),
                song.sample_rate_hz.to_string(),
                song.file_size_bytes.to_string(),
                song.song_duration_s.to_string(),
                song.num_plays.to_string(),
                itunesdb_helpers::decode_itunes_stars(song.song_rating_raw),
                song.song_added_to_library_ts.to_string(),
                song.song_added_to_library_epoch.to_string(),
                song.song_composer.to_string(),
                song.song_comment.to_string(),
                // Device info
                song.ipod_model.to_string(),
                song.ipod_generation.to_string(),
                song.ipod_name.to_string(),
                song.ipod_release_year.map_or("Unknown".to_string(), |y| y.to_string()),
            ])
            .expect("Can't write row to CSV");
    }
}