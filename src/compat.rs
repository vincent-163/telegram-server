//! Protocol-valid default responses for the broad Telegram API surface.
//!
//! Generated from the `grammers-tl-types` schema: every method below
//! returns a well-formed TL object of its declared return type so that
//! official clients can boot and exercise their full UI against this
//! single-node server. Methods with dedicated handlers in `rpc.rs` are
//! dispatched before this fallback.
//!
//! Deliberately excluded (explicit RPC error instead): payments, phone
//! calls, SMS jobs, premium, fragment and AI compose. Registration is
//! administrator-driven, so `auth.signUp` is rejected rather than stubbed.
//!
//! Regenerate with `tools/gen_compat.py` after upgrading
//! `grammers-tl-types`.

#![allow(clippy::all)]

use grammers_tl_types as tl;
use grammers_tl_types::Serializable as _;

/// Number of methods covered by the generated compatibility surface.
pub const DEFAULT_RESPONSE_METHODS: usize = 660;

/// Namespaces intentionally left unimplemented: they either move real money
/// (payments, premium, fragment), depend on telephony infrastructure (phone,
/// smsjobs) or require a hosted model (aicompose).
pub const UNSUPPORTED_NAMESPACES: &[&str] = &[
    "aicompose",
    "fragment",
    "payments",
    "phone",
    "premium",
    "smsjobs",
];

/// Return a default, well-formed response body for `ctor`, if the method is
/// part of the supported compatibility surface.
pub fn default_response(ctor: u32) -> Option<Vec<u8>> {
    let body = match ctor {
        0x3173d78 => tl::enums::updates::ChannelDifference::Empty(
            tl::types::updates::ChannelDifferenceEmpty {
                r#final: false,
                pts: 0,
                timeout: None,
            },
        )
        .to_bytes(),
        0x32da4cf => {
            tl::enums::account::EmailVerified::Verified(tl::types::account::EmailVerified {
                email: String::new(),
            })
            .to_bytes()
        }
        0x330e77f => tl::enums::Updates::TooLong.to_bytes(),
        0x388a3b5 => tl::enums::photos::Photo::Photo(tl::types::photos::Photo {
            photo: tl::enums::Photo::Empty(tl::types::PhotoEmpty { id: 0 }),
            users: Vec::new(),
        })
        .to_bytes(),
        0x38a08d3 => tl::enums::help::UserInfo::Empty.to_bytes(),
        0x4f1aaa9 => tl::enums::messages::FavedStickers::NotModified.to_bytes(),
        0x517165a => true.to_bytes(),
        0x53ca973 => true.to_bytes(),
        0x589ee75 => tl::enums::Updates::TooLong.to_bytes(),
        0x5a954c0 => Vec::<tl::enums::ReceivedNotifyMessage>::new().to_bytes(),
        0x5f58d0f => tl::enums::contacts::Found::Found(tl::types::contacts::Found {
            my_results: Vec::new(),
            results: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x62dd747 => {
            tl::enums::messages::AffectedHistory::History(tl::types::messages::AffectedHistory {
                pts: 0,
                pts_count: 0,
                offset: 0,
            })
            .to_bytes()
        }
        0x6dd654c => {
            tl::enums::ReactionsNotifySettings::Settings(tl::types::ReactionsNotifySettings {
                messages_notify_from: None,
                stories_notify_from: None,
                poll_votes_notify_from: None,
                sound: tl::enums::NotificationSound::None,
                show_previews: false,
            })
            .to_bytes()
        }
        0x6de6392 => true.to_bytes(),
        0x7967d36 => tl::enums::account::WallPapers::NotModified.to_bytes(),
        0x81202c9 => tl::enums::Updates::TooLong.to_bytes(),
        0x8736a09 => tl::enums::messages::ChatFull::Full(tl::types::messages::ChatFull {
            full_chat: tl::enums::ChatFull::Full(tl::types::ChatFull {
                can_set_username: false,
                has_scheduled: false,
                translations_disabled: false,
                id: 0,
                about: String::new(),
                participants: tl::enums::ChatParticipants::Forbidden(
                    tl::types::ChatParticipantsForbidden {
                        chat_id: 0,
                        self_participant: None,
                    },
                ),
                chat_photo: None,
                notify_settings: tl::enums::PeerNotifySettings::Settings(
                    tl::types::PeerNotifySettings {
                        show_previews: None,
                        silent: None,
                        mute_until: None,
                        ios_sound: None,
                        android_sound: None,
                        other_sound: None,
                        stories_muted: None,
                        stories_hide_sender: None,
                        stories_ios_sound: None,
                        stories_android_sound: None,
                        stories_other_sound: None,
                    },
                ),
                exported_invite: None,
                bot_info: None,
                pinned_msg_id: None,
                folder_id: None,
                call: None,
                ttl_period: None,
                groupcall_default_join_as: None,
                theme_emoticon: None,
                requests_pending: None,
                recent_requesters: None,
                available_reactions: None,
                reactions_limit: None,
            }),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x87fc5e7 => tl::enums::DataJson::Json(tl::types::DataJson {
            data: String::new(),
        })
        .to_bytes(),
        0x8fc711d => {
            tl::enums::AccountDaysTtl::Ttl(tl::types::AccountDaysTtl { days: 0 }).to_bytes()
        }
        0x96a0e00 => tl::enums::Updates::TooLong.to_bytes(),
        0x9c2dd95 => true.to_bytes(),
        0x9e82039 => tl::enums::photos::Photo::Photo(tl::types::photos::Photo {
            photo: tl::enums::Photo::Empty(tl::types::PhotoEmpty { id: 0 }),
            users: Vec::new(),
        })
        .to_bytes(),
        0xa245dd3 => true.to_bytes(),
        0xa4314f5 => {
            tl::enums::WebViewMessageSent::Sent(tl::types::WebViewMessageSent { msg_id: None })
                .to_bytes()
        }
        0xa7f6bbb => {
            tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
                .to_bytes()
        }
        0xb290c69 => tl::enums::Updates::TooLong.to_bytes(),
        0xb297e9b => true.to_bytes(),
        0xd36bf79 => true.to_bytes(),
        0xd91a548 => Vec::<tl::enums::User>::new().to_bytes(),
        0xe306d3a => {
            tl::enums::messages::AffectedMessages::Messages(tl::types::messages::AffectedMessages {
                pts: 0,
                pts_count: 0,
            })
            .to_bytes()
        }
        0xe7841f0 => tl::enums::Updates::TooLong.to_bytes(),
        0xecc2618 => tl::enums::Updates::TooLong.to_bytes(),
        0xecf6736 => tl::enums::messages::FeaturedStickers::NotModified(
            tl::types::messages::FeaturedStickersNotModified { count: 0 },
        )
        .to_bytes(),
        0xf578105 => tl::enums::account::EmojiStatuses::NotModified.to_bytes(),
        0xf635e1b => tl::enums::messages::HighScores::Scores(tl::types::messages::HighScores {
            scores: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x1013fd9e => true.to_bytes(),
        0x107e31a0 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x10cf3123 => true.to_bytes(),
        0x10e6bd2c => true.to_bytes(),
        0x10ea6184 => tl::enums::Updates::TooLong.to_bytes(),
        0x11e831ee => {
            tl::enums::messages::InactiveChats::Chats(tl::types::messages::InactiveChats {
                dates: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0x124b1c00 => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        0x12b3ad31 => tl::enums::PeerNotifySettings::Settings(tl::types::PeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until: None,
            ios_sound: None,
            android_sound: None,
            other_sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        })
        .to_bytes(),
        0x12cbf0c4 => tl::enums::channels::SponsoredMessageReportResult::ChooseOption(
            tl::types::channels::SponsoredMessageReportResultChooseOption {
                title: String::new(),
                options: Vec::new(),
            },
        )
        .to_bytes(),
        0x13005788 => tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }).to_bytes(),
        0x1359f4e6 => true.to_bytes(),
        0x13704a7c => tl::enums::Updates::TooLong.to_bytes(),
        0x139f63fb => true.to_bytes(),
        0x14967978 => tl::enums::MessageMedia::Empty.to_bytes(),
        0x1508b6af => {
            tl::enums::EmojiKeywordsDifference::Difference(tl::types::EmojiKeywordsDifference {
                lang_code: String::new(),
                from_version: 0,
                version: 0,
                keywords: Vec::new(),
            })
            .to_bytes()
        }
        0x15ad9f64 => true.to_bytes(),
        0x167fc0a1 => tl::enums::Updates::TooLong.to_bytes(),
        0x16fcc2cb => tl::enums::AttachMenuBots::NotModified.to_bytes(),
        0x1720b4d8 => {
            tl::enums::messages::AffectedHistory::History(tl::types::messages::AffectedHistory {
                pts: 0,
                pts_count: 0,
                offset: 0,
            })
            .to_bytes()
        }
        0x175df251 => tl::enums::Updates::TooLong.to_bytes(),
        0x17aeb75a => tl::enums::BotPreviewMedia::Media(tl::types::BotPreviewMedia {
            date: 0,
            media: tl::enums::MessageMedia::Empty,
        })
        .to_bytes(),
        0x18201aae => true.to_bytes(),
        0x182e6d6f => tl::enums::account::WebAuthorizations::Authorizations(
            tl::types::account::WebAuthorizations {
                authorizations: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0x18dea0ac => tl::enums::messages::AvailableReactions::NotModified.to_bytes(),
        0x19ba4a67 => tl::enums::account::PaidMessagesRevenue::Revenue(
            tl::types::account::PaidMessagesRevenue { stars_amount: 0 },
        )
        .to_bytes(),
        0x19bc4b6d => tl::enums::Updates::TooLong.to_bytes(),
        0x19c2f763 => tl::enums::updates::Difference::Empty(tl::types::updates::DifferenceEmpty {
            date: 0,
            seq: 0,
        })
        .to_bytes(),
        0x19d8eb45 => tl::enums::ReportResult::ChooseOption(tl::types::ReportResultChooseOption {
            title: String::new(),
            options: Vec::new(),
        })
        .to_bytes(),
        0x1ad4a04a => true.to_bytes(),
        0x1ae373ac => true.to_bytes(),
        0x1b3faa88 => tl::enums::auth::SentCode::Code(tl::types::auth::SentCode {
            r#type: tl::enums::auth::SentCodeType::App(tl::types::auth::SentCodeTypeApp {
                length: 0,
            }),
            phone_code_hash: String::new(),
            next_type: None,
            timeout: None,
        })
        .to_bytes(),
        0x1bbcf300 => Vec::<tl::enums::messages::SearchCounter>::new().to_bytes(),
        0x1bf89d74 => tl::enums::Updates::TooLong.to_bytes(),
        0x1c3db333 => tl::enums::Document::Empty(tl::types::DocumentEmpty { id: 0 }).to_bytes(),
        0x1cff7e08 => Vec::<tl::enums::MessageRange>::new().to_bytes(),
        0x1d2652ee => true.to_bytes(),
        0x1dd840f5 => tl::enums::messages::EmojiGroups::NotModified.to_bytes(),
        0x1e251c95 => true.to_bytes(),
        0x1e91fc99 => tl::enums::messages::SavedDialogs::NotModified(
            tl::types::messages::SavedDialogsNotModified { count: 0 },
        )
        .to_bytes(),
        0x1edaaac2 => {
            tl::enums::GlobalPrivacySettings::Settings(tl::types::GlobalPrivacySettings {
                archive_and_mute_new_noncontact_peers: false,
                keep_archived_unmuted: false,
                keep_archived_folders: false,
                hide_read_marks: false,
                new_noncontact_peers_require_premium: false,
                display_gifts_button: false,
                noncontact_peers_paid_stars: None,
                disallowed_gifts: None,
            })
            .to_bytes()
        }
        0x1f040578 => true.to_bytes(),
        0x1fb33026 => tl::enums::NearestDc::Dc(tl::types::NearestDc {
            country: String::new(),
            this_dc: 0,
            nearest_dc: 0,
        })
        .to_bytes(),
        0x21202222 => Vec::<tl::enums::DialogPeer>::new().to_bytes(),
        0x213853a3 => tl::enums::bots::AccessSettings::Settings(tl::types::bots::AccessSettings {
            restricted: false,
            add_users: None,
        })
        .to_bytes(),
        0x21a548f3 => tl::enums::messages::EmojiGroups::NotModified.to_bytes(),
        0x21a61057 => tl::enums::Updates::TooLong.to_bytes(),
        0x22567115 => tl::enums::SearchPostsFlood::Flood(tl::types::SearchPostsFlood {
            query_is_free: false,
            total_daily: 0,
            remains: 0,
            wait_till: None,
            stars_amount: 0,
        })
        .to_bytes(),
        0x22ddd30c => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x2442485e => true.to_bytes(),
        0x24e6818d => tl::enums::upload::WebFile::File(tl::types::upload::WebFile {
            size: 0,
            mime_type: String::new(),
            file_type: tl::enums::storage::FileType::FileUnknown,
            mtime: 0,
            bytes: Vec::new(),
        })
        .to_bytes(),
        0x25a71742 => {
            tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
                .to_bytes()
        }
        0x25b3eac7 => tl::enums::stories::Albums::NotModified.to_bytes(),
        0x269dc2c1 => tl::enums::WebViewResult::Url(tl::types::WebViewResultUrl {
            fullsize: false,
            fullscreen: false,
            same_origin: false,
            query_id: None,
            url: String::new(),
        })
        .to_bytes(),
        0x269e3643 => true.to_bytes(),
        0x269e9a49 => {
            tl::enums::messages::TranscribedAudio::Audio(tl::types::messages::TranscribedAudio {
                pending: false,
                transcription_id: 0,
                text: String::new(),
                trial_remains_num: None,
                trial_remains_until_date: None,
            })
            .to_bytes()
        }
        0x26cf8950 => {
            tl::enums::messages::DhConfig::NotModified(tl::types::messages::DhConfigNotModified {
                random: Vec::new(),
            })
            .to_bytes()
        }
        0x2714d86c => true.to_bytes(),
        0x284b3639 => true.to_bytes(),
        0x28e16cc8 => tl::enums::stories::StoryViews::Views(tl::types::stories::StoryViews {
            views: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x29a8962c => tl::enums::Updates::TooLong.to_bytes(),
        0x29b1c66a => tl::enums::messages::FoundStickers::NotModified(
            tl::types::messages::FoundStickersNotModified { next_offset: None },
        )
        .to_bytes(),
        0x29ee847a => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x2a862092 => tl::enums::MessageMedia::Empty.to_bytes(),
        0x2bf40ccc => tl::enums::Theme::Theme(tl::types::Theme {
            creator: false,
            default: false,
            for_chat: false,
            id: 0,
            access_hash: 0,
            slug: String::new(),
            title: String::new(),
            document: None,
            settings: None,
            emoticon: None,
            installs_count: None,
        })
        .to_bytes(),
        0x2c11c0d7 => tl::enums::EmojiList::NotModified.to_bytes(),
        0x2c4ada50 => tl::enums::stories::PeerStories::Stories(tl::types::stories::PeerStories {
            stories: tl::enums::PeerStories::Stories(tl::types::PeerStories {
                peer: tl::enums::Peer::User(tl::types::PeerUser { user_id: 0 }),
                max_read_id: None,
                stories: Vec::new(),
            }),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x2c63a72b => tl::enums::Updates::TooLong.to_bytes(),
        0x2c800be5 => {
            tl::enums::contacts::ImportedContacts::Contacts(tl::types::contacts::ImportedContacts {
                imported: Vec::new(),
                popular_invites: Vec::new(),
                retry_contacts: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0x2ca51fd1 => tl::enums::help::TermsOfServiceUpdate::Empty(
            tl::types::help::TermsOfServiceUpdateEmpty { expires: 0 },
        )
        .to_bytes(),
        0x2d0135b3 => true.to_bytes(),
        0x2d01b9ef => true.to_bytes(),
        0x2db873a9 => {
            tl::enums::auth::Authorization::Authorization(tl::types::auth::Authorization {
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            })
            .to_bytes()
        }
        0x2e2e8734 => true.to_bytes(),
        0x2e7b4543 => tl::enums::account::EmojiStatuses::NotModified.to_bytes(),
        0x2ecd56cd => tl::enums::messages::EmojiGroups::NotModified.to_bytes(),
        0x2f98c3d5 => tl::enums::Updates::TooLong.to_bytes(),
        0x30eb63f0 => {
            tl::enums::stories::CanSendStoryCount::Count(tl::types::stories::CanSendStoryCount {
                count_remains: 0,
            })
            .to_bytes()
        }
        0x316ce548 => {
            tl::enums::ReactionsNotifySettings::Settings(tl::types::ReactionsNotifySettings {
                messages_notify_from: None,
                stories_notify_from: None,
                poll_votes_notify_from: None,
                sound: tl::enums::NotificationSound::None,
                show_previews: false,
            })
            .to_bytes()
        }
        0x31813cd8 => true.to_bytes(),
        0x31a2a35e => tl::enums::bots::RequestedButton::Button(tl::types::bots::RequestedButton {
            webapp_req_id: String::new(),
        })
        .to_bytes(),
        0x31c1c44f => Vec::<tl::enums::ReadParticipantDate>::new().to_bytes(),
        0x327a30cb => true.to_bytes(),
        0x32d439a4 => tl::enums::messages::SentEncryptedMessage::Message(
            tl::types::messages::SentEncryptedMessage { date: 0 },
        )
        .to_bytes(),
        0x33ddf480 => {
            tl::enums::channels::AdminLogResults::Results(tl::types::channels::AdminLogResults {
                events: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0x34090c3b => {
            tl::enums::messages::HistoryImport::Import(tl::types::messages::HistoryImport { id: 0 })
                .to_bytes()
        }
        0x34fdc5c3 => tl::enums::messages::BotApp::App(tl::types::messages::BotApp {
            inactive: false,
            request_write_access: false,
            has_settings: false,
            app: tl::enums::BotApp::NotModified,
        })
        .to_bytes(),
        0x3514b3de => true.to_bytes(),
        0x35436bbc => true.to_bytes(),
        0x35705b8a => tl::enums::messages::FoundStickerSets::NotModified.to_bytes(),
        0x3583fcb1 => true.to_bytes(),
        0x35a0e062 => {
            tl::enums::EmojiKeywordsDifference::Difference(tl::types::EmojiKeywordsDifference {
                lang_code: String::new(),
                from_version: 0,
                version: 0,
                keywords: Vec::new(),
            })
            .to_bytes()
        }
        0x35a9e0d5 => tl::enums::EmojiList::NotModified.to_bytes(),
        0x35ddd674 => tl::enums::Updates::TooLong.to_bytes(),
        0x3637e05b => tl::enums::messages::SavedReactionTags::NotModified.to_bytes(),
        0x367544db => {
            tl::enums::messages::AffectedHistory::History(tl::types::messages::AffectedHistory {
                pts: 0,
                pts_count: 0,
                offset: 0,
            })
            .to_bytes()
        }
        0x36a73f77 => {
            tl::enums::messages::AffectedMessages::Messages(tl::types::messages::AffectedMessages {
                pts: 0,
                pts_count: 0,
            })
            .to_bytes()
        }
        0x36e5bf4d => {
            tl::enums::messages::AffectedHistory::History(tl::types::messages::AffectedHistory {
                pts: 0,
                pts_count: 0,
                offset: 0,
            })
            .to_bytes()
        }
        0x37096c70 => {
            tl::enums::auth::Authorization::Authorization(tl::types::auth::Authorization {
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            })
            .to_bytes()
        }
        0x374fef40 => tl::enums::stats::StoryStats::Stats(tl::types::stats::StoryStats {
            views_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            reactions_by_emotion_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
        })
        .to_bytes(),
        0x38df3532 => true.to_bytes(),
        0x3920e6ef => tl::enums::messages::ChatAdminsWithInvites::Invites(
            tl::types::messages::ChatAdminsWithInvites {
                admins: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0x392718f8 => true.to_bytes(),
        0x39461db2 => tl::enums::messages::Reactions::NotModified.to_bytes(),
        0x395f69da => {
            tl::enums::upload::CdnFile::ReuploadNeeded(tl::types::upload::CdnFileReuploadNeeded {
                request_token: Vec::new(),
            })
            .to_bytes()
        }
        0x3a5869ec => tl::enums::Theme::Theme(tl::types::Theme {
            creator: false,
            default: false,
            for_chat: false,
            id: 0,
            access_hash: 0,
            slug: String::new(),
            title: String::new(),
            document: None,
            settings: None,
            emoticon: None,
            installs_count: None,
        })
        .to_bytes(),
        0x3b1adf37 => true.to_bytes(),
        0x3b7d0ea6 => tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }).to_bytes(),
        0x3ba47bff => tl::enums::messages::ForumTopics::Topics(tl::types::messages::ForumTopics {
            order_by_create_date: false,
            count: 0,
            topics: Vec::new(),
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
            pts: 0,
        })
        .to_bytes(),
        0x3cc04740 => true.to_bytes(),
        0x3cd930b7 => true.to_bytes(),
        0x3d6ce850 => tl::enums::messages::SponsoredMessages::Empty.to_bytes(),
        0x3d8de0f9 => true.to_bytes(),
        0x3dbc0415 => {
            tl::enums::EncryptedChat::Empty(tl::types::EncryptedChatEmpty { id: 0 }).to_bytes()
        }
        0x3dc0f114 => tl::enums::help::RecentMeUrls::Urls(tl::types::help::RecentMeUrls {
            urls: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x3dea5b03 => tl::enums::account::SavedRingtone::Ringtone.to_bytes(),
        0x3e0bdd7c => tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }).to_bytes(),
        0x3e72ba19 => tl::enums::auth::LoggedOut::Out(tl::types::auth::LoggedOut {
            future_auth_token: None,
        })
        .to_bytes(),
        0x3eadb1bb => tl::enums::ChatInvite::Already(tl::types::ChatInviteAlready {
            chat: tl::enums::Chat::Empty(tl::types::ChatEmpty { id: 0 }),
        })
        .to_bytes(),
        0x3f64c076 => true.to_bytes(),
        0x3fedc75f => tl::enums::help::DeepLinkInfo::Empty.to_bytes(),
        0x3ff75734 => tl::enums::Updates::TooLong.to_bytes(),
        0x40582bb2 => true.to_bytes(),
        0x4067c5e6 => true.to_bytes(),
        0x40f48462 => true.to_bytes(),
        0x413a3e73 => tl::enums::WebViewResult::Url(tl::types::WebViewResultUrl {
            fullsize: false,
            fullscreen: false,
            same_origin: false,
            query_id: None,
            url: String::new(),
        })
        .to_bytes(),
        0x418d549c => tl::enums::Updates::TooLong.to_bytes(),
        0x41c10fff => tl::enums::chatlists::ChatlistInvite::Already(
            tl::types::chatlists::ChatlistInviteAlready {
                filter_id: 0,
                missing_peers: Vec::new(),
                already_peers: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0x423ab3ad => tl::enums::bots::PreviewInfo::Info(tl::types::bots::PreviewInfo {
            media: Vec::new(),
            lang_codes: Vec::new(),
        })
        .to_bytes(),
        0x429547e8 => tl::enums::account::PasskeyRegistrationOptions::Options(
            tl::types::account::PasskeyRegistrationOptions {
                options: tl::enums::DataJson::Json(tl::types::DataJson {
                    data: String::new(),
                }),
            },
        )
        .to_bytes(),
        0x42c6978f => Vec::<tl::enums::LangPackLanguage>::new().to_bytes(),
        0x43286cf2 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x435885b5 => true.to_bytes(),
        0x43fe19f3 => tl::enums::messages::HistoryImportParsed::Parsed(
            tl::types::messages::HistoryImportParsed {
                pm: false,
                group: false,
                title: None,
            },
        )
        .to_bytes(),
        0x4423e6c5 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x446972fd => tl::enums::messages::DiscussionMessage::Message(
            tl::types::messages::DiscussionMessage {
                messages: Vec::new(),
                max_id: None,
                read_inbox_max_id: None,
                read_outbox_max_id: None,
                unread_count: 0,
                chats: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0x449e0b51 => tl::enums::account::TmpPassword::Password(tl::types::account::TmpPassword {
            tmp_password: Vec::new(),
            valid_until: 0,
        })
        .to_bytes(),
        0x44fa7a15 => tl::enums::messages::SentEncryptedMessage::Message(
            tl::types::messages::SentEncryptedMessage { date: 0 },
        )
        .to_bytes(),
        0x4504d54f => true.to_bytes(),
        0x461b3f48 => tl::enums::messages::MessageReactionsList::List(
            tl::types::messages::MessageReactionsList {
                count: 0,
                reactions: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
                next_offset: None,
            },
        )
        .to_bytes(),
        0x4696459a => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        0x472455aa => tl::enums::Updates::TooLong.to_bytes(),
        0x49b30240 => tl::enums::help::TimezonesList::NotModified.to_bytes(),
        0x49e9528f => {
            tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
                .to_bytes()
        }
        0x4b00e066 => true.to_bytes(),
        0x4b0c8c0f => true.to_bytes(),
        0x4b12327b => tl::enums::Updates::TooLong.to_bytes(),
        0x4bc6589a => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x4c9409f6 => true.to_bytes(),
        0x4d392343 => tl::enums::help::InviteText::Text(tl::types::help::InviteText {
            message: String::new(),
        })
        .to_bytes(),
        0x4dafc503 => {
            tl::enums::stickers::SuggestedShortName::Name(tl::types::stickers::SuggestedShortName {
                short_name: String::new(),
            })
            .to_bytes()
        }
        0x4dc5085f => {
            tl::enums::messages::AffectedHistory::History(tl::types::messages::AffectedHistory {
                pts: 0,
                pts_count: 0,
                offset: 0,
            })
            .to_bytes()
        }
        0x4dd3a7f6 => true.to_bytes(),
        0x4e9963b2 => Vec::<tl::enums::EmojiLanguage>::new().to_bytes(),
        0x4ea4c80f => tl::enums::account::ConnectedBots::Bots(tl::types::account::ConnectedBots {
            connected_bots: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x4f47a016 => true.to_bytes(),
        0x4facb138 => true.to_bytes(),
        0x50077589 => true.to_bytes(),
        0x501569cf => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x5057c497 => tl::enums::EncryptedFile::Empty.to_bytes(),
        0x50f24105 => true.to_bytes(),
        0x514e999d => tl::enums::messages::BotResults::Results(tl::types::messages::BotResults {
            gallery: false,
            query_id: 0,
            next_offset: None,
            switch_pm: None,
            switch_webview: None,
            results: Vec::new(),
            cache_time: 0,
            users: Vec::new(),
        })
        .to_bytes(),
        0x518ad0b7 => {
            tl::enums::auth::PasskeyLoginOptions::Options(tl::types::auth::PasskeyLoginOptions {
                options: tl::enums::DataJson::Json(tl::types::DataJson {
                    data: String::new(),
                }),
            })
            .to_bytes()
        }
        0x52029342 => tl::enums::CdnConfig::Config(tl::types::CdnConfig {
            public_keys: Vec::new(),
        })
        .to_bytes(),
        0x522d5a7d => tl::enums::help::AppUpdate::Update(tl::types::help::AppUpdate {
            can_not_skip: false,
            id: 0,
            version: String::new(),
            text: String::new(),
            entities: Vec::new(),
            document: None,
            url: None,
            sticker: None,
        })
        .to_bytes(),
        0x53577479 => tl::enums::Updates::TooLong.to_bytes(),
        0x53618bce => tl::enums::WebViewResult::Url(tl::types::WebViewResultUrl {
            fullsize: false,
            fullscreen: false,
            same_origin: false,
            query_id: None,
            url: String::new(),
        })
        .to_bytes(),
        0x53bc0020 => true.to_bytes(),
        0x548a30f5 => tl::enums::account::Password::Password(tl::types::account::Password {
            has_recovery: false,
            has_secure_values: false,
            has_password: false,
            current_algo: None,
            srp_b: None,
            srp_id: None,
            hint: None,
            email_unconfirmed_pattern: None,
            new_algo: tl::enums::PasswordKdfAlgo::Unknown,
            new_secure_algo: tl::enums::SecurePasswordKdfAlgo::Unknown,
            secure_random: Vec::new(),
            pending_reset_date: None,
            login_email_pattern: None,
        })
        .to_bytes(),
        0x5492e5ee => tl::enums::account::ResolvedBusinessChatLinks::Links(
            tl::types::account::ResolvedBusinessChatLinks {
                peer: tl::enums::Peer::User(tl::types::PeerUser { user_id: 0 }),
                message: String::new(),
                entities: None,
                chats: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0x5559481d => tl::enums::messages::SentEncryptedMessage::Message(
            tl::types::messages::SentEncryptedMessage { date: 0 },
        )
        .to_bytes(),
        0x55a5bb66 => Vec::<i64>::new().to_bytes(),
        0x55b41fd6 => tl::enums::Passkey::Passkey(tl::types::Passkey {
            id: String::new(),
            name: String::new(),
            date: 0,
            software_emoji_id: None,
            last_usage_date: None,
        })
        .to_bytes(),
        0x55fb0996 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x56655768 => tl::enums::account::WebBrowserSettings::NotModified.to_bytes(),
        0x566decd0 => tl::enums::Updates::TooLong.to_bytes(),
        0x56987bd5 => true.to_bytes(),
        0x56da0b3f => tl::enums::account::AutoDownloadSettings::Settings(
            tl::types::account::AutoDownloadSettings {
                low: tl::enums::AutoDownloadSettings::Settings(tl::types::AutoDownloadSettings {
                    disabled: false,
                    video_preload_large: false,
                    audio_preload_next: false,
                    phonecalls_less_data: false,
                    stories_preload: false,
                    photo_size_max: 0,
                    video_size_max: 0,
                    file_size_max: 0,
                    video_upload_maxbitrate: 0,
                    small_queue_active_operations_max: 0,
                    large_queue_active_operations_max: 0,
                }),
                medium: tl::enums::AutoDownloadSettings::Settings(
                    tl::types::AutoDownloadSettings {
                        disabled: false,
                        video_preload_large: false,
                        audio_preload_next: false,
                        phonecalls_less_data: false,
                        stories_preload: false,
                        photo_size_max: 0,
                        video_size_max: 0,
                        file_size_max: 0,
                        video_upload_maxbitrate: 0,
                        small_queue_active_operations_max: 0,
                        large_queue_active_operations_max: 0,
                    },
                ),
                high: tl::enums::AutoDownloadSettings::Settings(tl::types::AutoDownloadSettings {
                    disabled: false,
                    video_preload_large: false,
                    audio_preload_next: false,
                    phonecalls_less_data: false,
                    stories_preload: false,
                    photo_size_max: 0,
                    video_size_max: 0,
                    file_size_max: 0,
                    video_upload_maxbitrate: 0,
                    small_queue_active_operations_max: 0,
                    large_queue_active_operations_max: 0,
                }),
            },
        )
        .to_bytes(),
        0x56e59f9c => tl::enums::auth::SentCode::Code(tl::types::auth::SentCode {
            r#type: tl::enums::auth::SentCodeType::App(tl::types::auth::SentCodeTypeApp {
                length: 0,
            }),
            phone_code_hash: String::new(),
            next_type: None,
            timeout: None,
        })
        .to_bytes(),
        0x570d6f6f => {
            tl::enums::messages::WebPagePreview::Preview(tl::types::messages::WebPagePreview {
                media: tl::enums::MessageMedia::Empty,
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0x5774ca74 => tl::enums::stories::Stories::Stories(tl::types::stories::Stories {
            count: 0,
            stories: Vec::new(),
            pinned_to_top: None,
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x5784d3e1 => tl::enums::messages::MessageViews::Views(tl::types::messages::MessageViews {
            views: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x57bbd166 => tl::enums::Updates::TooLong.to_bytes(),
        0x57f17692 => {
            tl::enums::messages::ArchivedStickers::Stickers(tl::types::messages::ArchivedStickers {
                count: 0,
                sets: Vec::new(),
            })
            .to_bytes()
        }
        0x5821a5dc => tl::enums::stories::Stories::Stories(tl::types::stories::Stories {
            count: 0,
            stories: Vec::new(),
            pinned_to_top: None,
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x58943ee2 => true.to_bytes(),
        0x58bbcb50 => tl::enums::Updates::TooLong.to_bytes(),
        0x58d6b376 => true.to_bytes(),
        0x58e4a740 => tl::enums::RpcDropAnswer::RpcAnswerUnknown.to_bytes(),
        0x58e63f6d => true.to_bytes(),
        0x59ae2b16 => tl::enums::Updates::TooLong.to_bytes(),
        0x5a6d7395 => true.to_bytes(),
        0x5b118126 => true.to_bytes(),
        0x5bd0ee50 => true.to_bytes(),
        0x5c003cef => true.to_bytes(),
        0x5cf09635 => tl::enums::messages::SavedGifs::NotModified.to_bytes(),
        0x5dc60f03 => tl::enums::messages::CheckedHistoryImportPeer::Peer(
            tl::types::messages::CheckedHistoryImportPeer {
                confirm_text: String::new(),
            },
        )
        .to_bytes(),
        0x5dd69e12 => tl::enums::contacts::Contacts::NotModified.to_bytes(),
        0x5dee78b0 => true.to_bytes(),
        0x5e437ed9 => true.to_bytes(),
        0x5e5259b6 => tl::enums::StoryAlbum::Album(tl::types::StoryAlbum {
            album_id: 0,
            title: String::new(),
            icon_photo: None,
            icon_video: None,
        })
        .to_bytes(),
        0x5f150144 => {
            tl::enums::stats::PublicForwards::Forwards(tl::types::stats::PublicForwards {
                count: 0,
                forwards: Vec::new(),
                next_offset: None,
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0x5f2178c3 => true.to_bytes(),
        0x60073674 => true.to_bytes(),
        0x60297dec => true.to_bytes(),
        0x60331907 => true.to_bytes(),
        0x60469778 => tl::enums::ResPq::Pq(tl::types::ResPq {
            nonce: [0u8; 16],
            server_nonce: [0u8; 16],
            pq: Vec::new(),
            server_public_key_fingerprints: Vec::new(),
        })
        .to_bytes(),
        0x60ed4229 => tl::enums::Updates::TooLong.to_bytes(),
        0x61e3f854 => tl::enums::help::AppConfig::NotModified.to_bytes(),
        0x621d5fa0 => tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
            token: String::new(),
        })
        .to_bytes(),
        0x63c66506 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x640f82b8 => tl::enums::messages::AllStickers::NotModified.to_bytes(),
        0x646e1097 => true.to_bytes(),
        0x64780b14 => tl::enums::messages::FeaturedStickers::NotModified(
            tl::types::messages::FeaturedStickersNotModified { count: 0 },
        )
        .to_bytes(),
        0x652e4400 => tl::enums::Theme::Theme(tl::types::Theme {
            creator: false,
            default: false,
            for_chat: false,
            id: 0,
            access_hash: 0,
            slug: String::new(),
            title: String::new(),
            document: None,
            settings: None,
            emoticon: None,
            installs_count: None,
        })
        .to_bytes(),
        0x653db63d => {
            tl::enums::ExportedChatlistInvite::Invite(tl::types::ExportedChatlistInvite {
                title: String::new(),
                url: String::new(),
                peers: Vec::new(),
            })
            .to_bytes()
        }
        0x658b7188 => {
            tl::enums::DefaultHistoryTtl::Ttl(tl::types::DefaultHistoryTtl { period: 0 }).to_bytes()
        }
        0x65ad71dc => Vec::<tl::enums::WallPaper>::new().to_bytes(),
        0x6628562c => true.to_bytes(),
        0x66a08c7e => tl::enums::Updates::TooLong.to_bytes(),
        0x66b91b70 => tl::enums::help::UserInfo::Empty.to_bytes(),
        0x66cdafc4 => true.to_bytes(),
        0x66e486fb => true.to_bytes(),
        0x67a3f0de => tl::enums::UrlAuthResult::Request(tl::types::UrlAuthResultRequest {
            request_write_access: false,
            request_phone_number: false,
            match_codes_first: false,
            is_app: false,
            bot: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            domain: String::new(),
            browser: None,
            platform: None,
            ip: None,
            region: None,
            match_codes: None,
            user_id_hint: None,
            verified_app_name: None,
        })
        .to_bytes(),
        0x67a3ff2c => {
            tl::enums::auth::Authorization::Authorization(tl::types::auth::Authorization {
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            })
            .to_bytes()
        }
        0x67ed1f68 => true.to_bytes(),
        0x682d2594 => true.to_bytes(),
        0x6847d0ab => tl::enums::Updates::TooLong.to_bytes(),
        0x684d214e => true.to_bytes(),
        0x68f3e4eb => tl::enums::Updates::TooLong.to_bytes(),
        0x69f59d69 => true.to_bytes(),
        0x6a0d3206 => true.to_bytes(),
        0x6a3f8d65 => tl::enums::Updates::TooLong.to_bytes(),
        0x6a596502 => tl::enums::LangPackLanguage::Language(tl::types::LangPackLanguage {
            official: false,
            rtl: false,
            beta: false,
            name: String::new(),
            native_name: String::new(),
            lang_code: String::new(),
            base_lang_code: None,
            plural_code: String::new(),
            strings_count: 0,
            translated_count: 0,
            translations_url: String::new(),
        })
        .to_bytes(),
        0x6a6e7854 => tl::enums::Updates::TooLong.to_bytes(),
        0x6aa3f6bd => tl::enums::messages::SearchResultsCalendar::Calendar(
            tl::types::messages::SearchResultsCalendar {
                inexact: false,
                count: 0,
                min_date: 0,
                min_msg_id: 0,
                offset_id_offset: None,
                periods: Vec::new(),
                messages: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0x6c5a5b37 => true.to_bytes(),
        0x6c5cf2a7 => tl::enums::Updates::TooLong.to_bytes(),
        0x6c750de1 => tl::enums::Updates::TooLong.to_bytes(),
        0x6e2be050 => {
            tl::enums::ChatOnlines::Onlines(tl::types::ChatOnlines { onlines: 0 }).to_bytes()
        }
        0x6f02f748 => true.to_bytes(),
        0x6f6f9c96 => tl::enums::messages::SavedDialogs::NotModified(
            tl::types::messages::SavedDialogsNotModified { count: 0 },
        )
        .to_bytes(),
        0x6f70dde1 => {
            tl::enums::account::BusinessChatLinks::Links(tl::types::account::BusinessChatLinks {
                links: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0x702a40e0 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x70c32edb => tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }).to_bytes(),
        0x719c5c5e => true.to_bytes(),
        0x7206e458 => tl::enums::account::Themes::NotModified.to_bytes(),
        0x725afbbc => tl::enums::contacts::ResolvedPeer::Peer(tl::types::contacts::ResolvedPeer {
            peer: tl::enums::Peer::User(tl::types::PeerUser { user_id: 0 }),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x735787a8 => tl::enums::help::CountriesList::NotModified.to_bytes(),
        0x73665bc2 => Vec::<tl::enums::SecureValue>::new().to_bytes(),
        0x73746f5c => tl::enums::messages::ExportedChatInvite::Invite(
            tl::types::messages::ExportedChatInvite {
                invite: tl::enums::ExportedChatInvite::ChatInviteExported(
                    tl::types::ChatInviteExported {
                        revoked: false,
                        permanent: false,
                        request_needed: false,
                        link: String::new(),
                        admin_id: 0,
                        date: 0,
                        start_date: None,
                        expire_date: None,
                        usage_limit: None,
                        usage: None,
                        requested: None,
                        subscription_expired: None,
                        title: None,
                        subscription_pricing: None,
                    },
                ),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0x73783ffd => tl::enums::Updates::TooLong.to_bytes(),
        0x7488ce5b => tl::enums::messages::EmojiGroups::NotModified.to_bytes(),
        0x74fae13a => tl::enums::Updates::TooLong.to_bytes(),
        0x7573a4e9 => {
            tl::enums::users::SavedMusic::NotModified(tl::types::users::SavedMusicNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x76a86270 => tl::enums::Updates::TooLong.to_bytes(),
        0x76f36233 => true.to_bytes(),
        0x77216192 => tl::enums::AttachMenuBotsBot::Bot(tl::types::AttachMenuBotsBot {
            bot: tl::enums::AttachMenuBot::Bot(tl::types::AttachMenuBot {
                inactive: false,
                has_settings: false,
                request_write_access: false,
                show_in_attach_menu: false,
                show_in_side_menu: false,
                side_menu_disclaimer_needed: false,
                bot_id: 0,
                short_name: String::new(),
                peer_types: None,
                icons: Vec::new(),
            }),
            users: Vec::new(),
        })
        .to_bytes(),
        0x7727a7d5 => tl::enums::account::EmojiStatuses::NotModified.to_bytes(),
        0x778b5ab3 => tl::enums::StarRefProgram::Program(tl::types::StarRefProgram {
            bot_id: 0,
            commission_permille: 0,
            duration_months: None,
            end_date: None,
            daily_revenue_per_user: None,
        })
        .to_bytes(),
        0x77ced9d0 => tl::enums::channels::ChannelParticipants::NotModified.to_bytes(),
        0x78337739 => true.to_bytes(),
        0x78499170 => Vec::<tl::enums::RecentStory>::new().to_bytes(),
        0x78515775 => tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }).to_bytes(),
        0x788464e1 => true.to_bytes(),
        0x788d7fe3 => {
            tl::enums::users::SavedMusic::NotModified(tl::types::users::SavedMusicNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x791451ed => true.to_bytes(),
        0x7a7f2a15 => true.to_bytes(),
        0x7abe77ec => tl::enums::Pong::Pong(tl::types::Pong {
            msg_id: 0,
            ping_id: 0,
        })
        .to_bytes(),
        0x7adc669d => Vec::<i32>::new().to_bytes(),
        0x7b8def20 => tl::enums::ExportedStoryLink::Link(tl::types::ExportedStoryLink {
            link: String::new(),
        })
        .to_bytes(),
        0x7c2557c4 => true.to_bytes(),
        0x7e58ee9c => true.to_bytes(),
        0x7e960193 => tl::enums::auth::SentCode::Code(tl::types::auth::SentCode {
            r#type: tl::enums::auth::SentCodeType::App(tl::types::auth::SentCodeTypeApp {
                length: 0,
            }),
            phone_code_hash: String::new(),
            next_type: None,
            timeout: None,
        })
        .to_bytes(),
        0x7ed094a1 => tl::enums::messages::FeaturedStickers::NotModified(
            tl::types::messages::FeaturedStickersNotModified { count: 0 },
        )
        .to_bytes(),
        0x7ed23c57 => {
            tl::enums::stories::StoryViewsList::List(tl::types::stories::StoryViewsList {
                count: 0,
                views_count: 0,
                forwards_count: 0,
                reactions_count: 0,
                views: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
                next_offset: None,
            })
            .to_bytes()
        }
        0x7f1d072f => true.to_bytes(),
        0x7f4b690a => true.to_bytes(),
        0x7f6a1e22 => tl::enums::messages::ChatInviteJoinResult::Ok(
            tl::types::messages::ChatInviteJoinResultOk {
                updates: tl::enums::Updates::TooLong,
            },
        )
        .to_bytes(),
        0x7fd736b2 => tl::enums::Updates::TooLong.to_bytes(),
        0x7fe7e815 => tl::enums::Updates::TooLong.to_bytes(),
        0x8107455c => tl::enums::Updates::TooLong.to_bytes(),
        0x8235057e => true.to_bytes(),
        0x82574ae5 => tl::enums::auth::SentCode::Code(tl::types::auth::SentCode {
            r#type: tl::enums::auth::SentCodeType::App(tl::types::auth::SentCodeTypeApp {
                length: 0,
            }),
            phone_code_hash: String::new(),
            next_type: None,
            timeout: None,
        })
        .to_bytes(),
        0x82f1e39f => Vec::<tl::enums::SavedContact>::new().to_bytes(),
        0x831a83a2 => tl::enums::Document::Empty(tl::types::DocumentEmpty { id: 0 }).to_bytes(),
        0x8341ecc0 => {
            tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
                .to_bytes()
        }
        0x8472478e => tl::enums::chatlists::ExportedChatlistInvite::Invite(
            tl::types::chatlists::ExportedChatlistInvite {
                filter: tl::enums::DialogFilter::Filter(tl::types::DialogFilter {
                    contacts: false,
                    non_contacts: false,
                    groups: false,
                    broadcasts: false,
                    bots: false,
                    exclude_muted: false,
                    exclude_read: false,
                    exclude_archived: false,
                    title_noanimate: false,
                    id: 0,
                    title: tl::enums::TextWithEntities::Entities(tl::types::TextWithEntities {
                        text: String::new(),
                        entities: Vec::new(),
                    }),
                    emoticon: None,
                    color: None,
                    pinned_peers: Vec::new(),
                    include_peers: Vec::new(),
                    exclude_peers: Vec::new(),
                }),
                invite: tl::enums::ExportedChatlistInvite::Invite(
                    tl::types::ExportedChatlistInvite {
                        title: String::new(),
                        url: String::new(),
                        peers: Vec::new(),
                    },
                ),
            },
        )
        .to_bytes(),
        0x84be5b93 => true.to_bytes(),
        0x84c1fd4e => {
            tl::enums::messages::AffectedMessages::Messages(tl::types::messages::AffectedMessages {
                pts: 0,
                pts_count: 0,
            })
            .to_bytes()
        }
        0x84f80814 => tl::enums::Updates::TooLong.to_bytes(),
        0x8514bdda => true.to_bytes(),
        0x8525606f => tl::enums::BotPreviewMedia::Media(tl::types::BotPreviewMedia {
            date: 0,
            media: tl::enums::MessageMedia::Empty,
        })
        .to_bytes(),
        0x8535fbd9 => true.to_bytes(),
        0x857ebdb8 => tl::enums::messages::PreparedInlineMessage::Message(
            tl::types::messages::PreparedInlineMessage {
                query_id: 0,
                result: tl::enums::BotInlineResult::Result(tl::types::BotInlineResult {
                    id: String::new(),
                    r#type: String::new(),
                    title: None,
                    description: None,
                    url: None,
                    thumb: None,
                    content: None,
                    send_message: tl::enums::BotInlineMessage::MediaAuto(
                        tl::types::BotInlineMessageMediaAuto {
                            invert_media: false,
                            message: String::new(),
                            entities: None,
                            reply_markup: None,
                        },
                    ),
                }),
                peer_types: Vec::new(),
                cache_time: 0,
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0x864b2581 => tl::enums::Updates::TooLong.to_bytes(),
        0x8653febe => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        0x86a0765d => tl::enums::account::WebBrowserSettings::NotModified.to_bytes(),
        0x87704394 => true.to_bytes(),
        0x879537f1 => true.to_bytes(),
        0x87cf7f2f => Vec::<i64>::new().to_bytes(),
        0x87f2219b => true.to_bytes(),
        0x8851e68e => tl::enums::BusinessChatLink::Link(tl::types::BusinessChatLink {
            link: String::new(),
            message: String::new(),
            entities: None,
            title: None,
            views: 0,
        })
        .to_bytes(),
        0x89419521 => {
            tl::enums::chatlists::ChatlistUpdates::Updates(tl::types::chatlists::ChatlistUpdates {
                missing_peers: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0x894cc99c => tl::enums::UrlAuthResult::Request(tl::types::UrlAuthResultRequest {
            request_write_access: false,
            request_phone_number: false,
            match_codes_first: false,
            is_app: false,
            bot: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            domain: String::new(),
            browser: None,
            platform: None,
            ip: None,
            region: None,
            match_codes: None,
            user_id_hint: None,
            verified_app_name: None,
        })
        .to_bytes(),
        0x8999602d => true.to_bytes(),
        0x899fe31d => tl::enums::SecureValue::Value(tl::types::SecureValue {
            r#type: tl::enums::SecureValueType::PersonalDetails,
            data: None,
            front_side: None,
            reverse_side: None,
            selfie: None,
            translation: None,
            files: None,
            plain_data: None,
            hash: Vec::new(),
        })
        .to_bytes(),
        0x8af94344 => tl::enums::contacts::ResolvedPeer::Peer(tl::types::contacts::ResolvedPeer {
            peer: tl::enums::Peer::User(tl::types::PeerUser { user_id: 0 }),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x8b716587 => true.to_bytes(),
        0x8b89dfbd => true.to_bytes(),
        0x8b9b4dae => {
            tl::enums::account::ContentSettings::Settings(tl::types::account::ContentSettings {
                sensitive_enabled: false,
                sensitive_can_change: false,
            })
            .to_bytes()
        }
        0x8bba90e6 => tl::enums::Updates::TooLong.to_bytes(),
        0x8c3410af => tl::enums::BusinessChatLink::Link(tl::types::BusinessChatLink {
            link: String::new(),
            message: String::new(),
            entities: None,
            title: None,
            views: 0,
        })
        .to_bytes(),
        0x8c4bfe5d => {
            tl::enums::OutboxReadDate::Date(tl::types::OutboxReadDate { date: 0 }).to_bytes()
        }
        0x8c5006f8 => true.to_bytes(),
        0x8d3456d0 => true.to_bytes(),
        0x8d52a951 => {
            tl::enums::auth::Authorization::Authorization(tl::types::auth::Authorization {
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            })
            .to_bytes()
        }
        0x8d9692a3 => tl::enums::messages::WebPage::Page(tl::types::messages::WebPage {
            webpage: tl::enums::WebPage::Empty(tl::types::WebPageEmpty { id: 0, url: None }),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x8e39261e => true.to_bytes(),
        0x8e48a188 => true.to_bytes(),
        0x8ef3eab0 => {
            tl::enums::account::Takeout::Takeout(tl::types::account::Takeout { id: 0 }).to_bytes()
        }
        0x8ef8ecc0 => tl::enums::Updates::TooLong.to_bytes(),
        0x8f9e6898 => tl::enums::Updates::TooLong.to_bytes(),
        0x8fdf1920 => true.to_bytes(),
        0x8ffacae1 => tl::enums::Updates::TooLong.to_bytes(),
        0x9021ab67 => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        0x90c894b5 => true.to_bytes(),
        0x91006707 => tl::enums::Updates::TooLong.to_bytes(),
        0x9156982a => Vec::<tl::enums::FileHash>::new().to_bytes(),
        0x915860ae => tl::enums::EmojiList::NotModified.to_bytes(),
        0x91cd32a8 => tl::enums::photos::Photos::Photos(tl::types::photos::Photos {
            photos: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x91dc3f31 => Vec::<tl::enums::FileHash>::new().to_bytes(),
        0x925ec9ea => true.to_bytes(),
        0x92b4494c => tl::enums::messages::FoundStickerSets::NotModified.to_bytes(),
        0x92ceddd4 => tl::enums::messages::InvitedUsers::Users(tl::types::messages::InvitedUsers {
            updates: tl::enums::Updates::TooLong,
            missing_invitees: Vec::new(),
        })
        .to_bytes(),
        0x9308ce1b => tl::enums::account::ResetPasswordResult::ResetPasswordFailedWait(
            tl::types::account::ResetPasswordFailedWait { retry_date: 0 },
        )
        .to_bytes(),
        0x9342ca07 => {
            tl::enums::messages::BotCallbackAnswer::Answer(tl::types::messages::BotCallbackAnswer {
                alert: false,
                has_url: false,
                native_ui: false,
                message: None,
                url: None,
                cache_time: 0,
            })
            .to_bytes()
        }
        0x94a495c3 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x94c65c76 => true.to_bytes(),
        0x95ac5ce4 => tl::enums::auth::LoginToken::Token(tl::types::auth::LoginToken {
            expires: 0,
            token: Vec::new(),
        })
        .to_bytes(),
        0x96e6cd81 => tl::enums::Updates::TooLong.to_bytes(),
        0x9709b1c2 => true.to_bytes(),
        0x973478b6 => tl::enums::contacts::TopPeers::NotModified.to_bytes(),
        0x9738bb15 => tl::enums::Updates::TooLong.to_bytes(),
        0x9857ad07 => {
            tl::enums::auth::Authorization::Authorization(tl::types::auth::Authorization {
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            })
            .to_bytes()
        }
        0x98e037bb => tl::enums::account::SentEmailCode::Code(tl::types::account::SentEmailCode {
            email_pattern: String::new(),
            length: 0,
        })
        .to_bytes(),
        0x998ab009 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0x9a75a1ef => Vec::<i32>::new().to_bytes(),
        0x9a868f80 => tl::enums::contacts::Blocked::Blocked(tl::types::contacts::Blocked {
            blocked: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0x9a98ad68 => tl::enums::Updates::TooLong.to_bytes(),
        0x9adf82fe => tl::enums::account::WebBrowserSettings::NotModified.to_bytes(),
        0x9ae91519 => tl::enums::Updates::TooLong.to_bytes(),
        0x9b2754a8 => Vec::<tl::enums::FileHash>::new().to_bytes(),
        0x9b5ae7f9 => tl::enums::Updates::TooLong.to_bytes(),
        0x9baa9647 => tl::enums::Updates::TooLong.to_bytes(),
        0x9c60eb28 => tl::enums::BotMenuButton::Default.to_bytes(),
        0x9c7f2f10 => tl::enums::messages::SearchResultsPositions::Positions(
            tl::types::messages::SearchResultsPositions {
                count: 0,
                positions: Vec::new(),
            },
        )
        .to_bytes(),
        0x9cd4eaf9 => {
            tl::enums::account::PasswordSettings::Settings(tl::types::account::PasswordSettings {
                email: None,
                secure_settings: None,
            })
            .to_bytes()
        }
        0x9cdf08cd => tl::enums::help::Support::Support(tl::types::help::Support {
            phone_number: String::new(),
            user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
        })
        .to_bytes(),
        0x9da9403b => tl::enums::messages::RecentStickers::NotModified.to_bytes(),
        0x9dfeefb4 => true.to_bytes(),
        0x9e6b131a => true.to_bytes(),
        0x9eb51445 => true.to_bytes(),
        0x9ec44f93 => {
            tl::enums::messages::AffectedHistory::History(tl::types::messages::AffectedHistory {
                pts: 0,
                pts_count: 0,
                offset: 0,
            })
            .to_bytes()
        }
        0x9f07c728 => true.to_bytes(),
        0x9fab0d1a => true.to_bytes(),
        0xa00f32b0 => tl::enums::Updates::TooLong.to_bytes(),
        0xa0ab6cc6 => tl::enums::channels::ChannelParticipant::Participant(
            tl::types::channels::ChannelParticipant {
                participant: tl::enums::ChannelParticipant::Participant(
                    tl::types::ChannelParticipant {
                        user_id: 0,
                        date: 0,
                        subscription_until_date: None,
                        rank: None,
                    },
                ),
                chats: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0xa0b80cf8 => true.to_bytes(),
        0xa0f4cb4f => {
            tl::enums::messages::Dialogs::NotModified(tl::types::messages::DialogsNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0xa1405817 => tl::enums::Updates::TooLong.to_bytes(),
        0xa1b70815 => {
            tl::enums::users::Users::Users(tl::types::users::Users { users: Vec::new() }).to_bytes()
        }
        0xa2185cab => tl::enums::Updates::TooLong.to_bytes(),
        0xa26a7fa5 => true.to_bytes(),
        0xa2875319 => tl::enums::Updates::TooLong.to_bytes(),
        0xa29cd42c => Vec::<tl::enums::DialogFilterSuggested>::new().to_bytes(),
        0xa2a5594d => Vec::<tl::enums::BotPreviewMedia>::new().to_bytes(),
        0xa2b5a3f6 => tl::enums::messages::ExportedChatInvites::Invites(
            tl::types::messages::ExportedChatInvites {
                count: 0,
                invites: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0xa2c0cf74 => true.to_bytes(),
        0xa36396e5 => tl::enums::StoryAlbum::Album(tl::types::StoryAlbum {
            album_id: 0,
            title: String::new(),
            icon_photo: None,
            icon_video: None,
        })
        .to_bytes(),
        0xa423bb51 => true.to_bytes(),
        0xa455de90 => {
            tl::enums::ExportedChatInvite::ChatInviteExported(tl::types::ChatInviteExported {
                revoked: false,
                permanent: false,
                request_needed: false,
                link: String::new(),
                admin_id: 0,
                date: 0,
                start_date: None,
                expire_date: None,
                usage_limit: None,
                usage: None,
                requested: None,
                subscription_expired: None,
                title: None,
                subscription_pricing: None,
            })
            .to_bytes()
        }
        0xa556dac8 => Vec::<i32>::new().to_bytes(),
        0xa56a8b60 => {
            tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
                .to_bytes()
        }
        0xa57a7dad => {
            tl::enums::auth::Authorization::Authorization(tl::types::auth::Authorization {
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            })
            .to_bytes()
        }
        0xa5866b41 => tl::enums::Updates::TooLong.to_bytes(),
        0xa59b102f => true.to_bytes(),
        0xa5a356f9 => tl::enums::auth::SentCode::Code(tl::types::auth::SentCode {
            r#type: tl::enums::auth::SentCodeType::App(tl::types::auth::SentCodeTypeApp {
                length: 0,
            }),
            phone_code_hash: String::new(),
            next_type: None,
            timeout: None,
        })
        .to_bytes(),
        0xa5eec345 => tl::enums::messages::TranslatedText::TranslateResult(
            tl::types::messages::TranslateResult { result: Vec::new() },
        )
        .to_bytes(),
        0xa60ab9ce => tl::enums::EmojiList::NotModified.to_bytes(),
        0xa614d034 => true.to_bytes(),
        0xa6437ef6 => {
            tl::enums::stats::PublicForwards::Forwards(tl::types::stats::PublicForwards {
                count: 0,
                forwards: Vec::new(),
                next_offset: None,
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0xa677244f => tl::enums::auth::SentCode::Code(tl::types::auth::SentCode {
            r#type: tl::enums::auth::SentCodeType::App(tl::types::auth::SentCodeTypeApp {
                length: 0,
            }),
            phone_code_hash: String::new(),
            next_type: None,
            timeout: None,
        })
        .to_bytes(),
        0xa6b1e39a => tl::enums::Updates::TooLong.to_bytes(),
        0xa731e257 => true.to_bytes(),
        0xa76a5392 => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        0xa850a693 => true.to_bytes(),
        0xa85bd1c2 => true.to_bytes(),
        0xa929597a => {
            tl::enums::account::AuthorizationForm::Form(tl::types::account::AuthorizationForm {
                required_types: Vec::new(),
                values: Vec::new(),
                errors: Vec::new(),
                users: Vec::new(),
                privacy_policy_url: None,
            })
            .to_bytes()
        }
        0xaa2769ed => tl::enums::DataJson::Json(tl::types::DataJson {
            data: String::new(),
        })
        .to_bytes(),
        0xaac7b717 => {
            tl::enums::auth::Authorization::Authorization(tl::types::auth::Authorization {
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            })
            .to_bytes()
        }
        0xab42441a => tl::enums::stats::BroadcastStats::Stats(tl::types::stats::BroadcastStats {
            period: tl::enums::StatsDateRangeDays::Days(tl::types::StatsDateRangeDays {
                min_date: 0,
                max_date: 0,
            }),
            followers: tl::enums::StatsAbsValueAndPrev::Prev(tl::types::StatsAbsValueAndPrev {
                current: 0.0,
                previous: 0.0,
            }),
            views_per_post: tl::enums::StatsAbsValueAndPrev::Prev(
                tl::types::StatsAbsValueAndPrev {
                    current: 0.0,
                    previous: 0.0,
                },
            ),
            shares_per_post: tl::enums::StatsAbsValueAndPrev::Prev(
                tl::types::StatsAbsValueAndPrev {
                    current: 0.0,
                    previous: 0.0,
                },
            ),
            reactions_per_post: tl::enums::StatsAbsValueAndPrev::Prev(
                tl::types::StatsAbsValueAndPrev {
                    current: 0.0,
                    previous: 0.0,
                },
            ),
            views_per_story: tl::enums::StatsAbsValueAndPrev::Prev(
                tl::types::StatsAbsValueAndPrev {
                    current: 0.0,
                    previous: 0.0,
                },
            ),
            shares_per_story: tl::enums::StatsAbsValueAndPrev::Prev(
                tl::types::StatsAbsValueAndPrev {
                    current: 0.0,
                    previous: 0.0,
                },
            ),
            reactions_per_story: tl::enums::StatsAbsValueAndPrev::Prev(
                tl::types::StatsAbsValueAndPrev {
                    current: 0.0,
                    previous: 0.0,
                },
            ),
            enabled_notifications: tl::enums::StatsPercentValue::Value(
                tl::types::StatsPercentValue {
                    part: 0.0,
                    total: 0.0,
                },
            ),
            growth_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            followers_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            mute_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            top_hours_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            interactions_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            iv_interactions_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            views_by_source_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            new_followers_by_source_graph: tl::enums::StatsGraph::Async(
                tl::types::StatsGraphAsync {
                    token: String::new(),
                },
            ),
            languages_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            reactions_by_emotion_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            story_interactions_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            story_reactions_by_emotion_graph: tl::enums::StatsGraph::Async(
                tl::types::StatsGraphAsync {
                    token: String::new(),
                },
            ),
            recent_posts_interactions: Vec::new(),
        })
        .to_bytes(),
        0xabbbd346 => tl::enums::TextWithEntities::Entities(tl::types::TextWithEntities {
            text: String::new(),
            entities: Vec::new(),
        })
        .to_bytes(),
        0xabcfa9fd => tl::enums::help::PeerColors::NotModified.to_bytes(),
        0xac806d61 => tl::enums::stories::Stories::Stories(tl::types::stories::Stories {
            count: 0,
            stories: Vec::new(),
            pinned_to_top: None,
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xac81bbde => true.to_bytes(),
        0xac8505a5 => tl::enums::Updates::TooLong.to_bytes(),
        0xad0fa15c => true.to_bytes(),
        0xad399cee => tl::enums::Updates::TooLong.to_bytes(),
        0xad8c9a23 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0xadcbbcda => {
            tl::enums::account::AutoSaveSettings::Settings(tl::types::account::AutoSaveSettings {
                users_settings: tl::enums::AutoSaveSettings::Settings(
                    tl::types::AutoSaveSettings {
                        photos: false,
                        videos: false,
                        video_max_size: None,
                    },
                ),
                chats_settings: tl::enums::AutoSaveSettings::Settings(
                    tl::types::AutoSaveSettings {
                        photos: false,
                        videos: false,
                        video_max_size: None,
                    },
                ),
                broadcasts_settings: tl::enums::AutoSaveSettings::Settings(
                    tl::types::AutoSaveSettings {
                        photos: false,
                        videos: false,
                        video_max_size: None,
                    },
                ),
                exceptions: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0xae59db5f => Vec::<i32>::new().to_bytes(),
        0xaeb00b34 => tl::enums::messages::ChatFull::Full(tl::types::messages::ChatFull {
            full_chat: tl::enums::ChatFull::Full(tl::types::ChatFull {
                can_set_username: false,
                has_scheduled: false,
                translations_disabled: false,
                id: 0,
                about: String::new(),
                participants: tl::enums::ChatParticipants::Forbidden(
                    tl::types::ChatParticipantsForbidden {
                        chat_id: 0,
                        self_participant: None,
                    },
                ),
                chat_photo: None,
                notify_settings: tl::enums::PeerNotifySettings::Settings(
                    tl::types::PeerNotifySettings {
                        show_previews: None,
                        silent: None,
                        mute_until: None,
                        ios_sound: None,
                        android_sound: None,
                        other_sound: None,
                        stories_muted: None,
                        stories_hide_sender: None,
                        stories_ios_sound: None,
                        stories_android_sound: None,
                        stories_other_sound: None,
                    },
                ),
                exported_invite: None,
                bot_info: None,
                pinned_msg_id: None,
                folder_id: None,
                call: None,
                ttl_period: None,
                groupcall_default_join_as: None,
                theme_emoticon: None,
                requests_pending: None,
                recent_requesters: None,
                available_reactions: None,
                reactions_limit: None,
            }),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xaf0a4a08 => tl::enums::messages::ForumTopics::Topics(tl::types::messages::ForumTopics {
            order_by_create_date: false,
            count: 0,
            topics: Vec::new(),
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
            pts: 0,
        })
        .to_bytes(),
        0xb0711d83 => Vec::<tl::enums::User>::new().to_bytes(),
        0xb08f922a => {
            tl::enums::messages::AffectedHistory::History(tl::types::messages::AffectedHistory {
                pts: 0,
                pts_count: 0,
                offset: 0,
            })
            .to_bytes()
        }
        0xb0d81a83 => true.to_bytes(),
        0xb106e66c => tl::enums::Updates::TooLong.to_bytes(),
        0xb1f2061f => tl::enums::Document::Empty(tl::types::DocumentEmpty { id: 0 }).to_bytes(),
        0xb2028afb => true.to_bytes(),
        0xb2081a35 => tl::enums::Updates::TooLong.to_bytes(),
        0xb26732a9 => true.to_bytes(),
        0xb288bc7d => Vec::<tl::enums::SecureValue>::new().to_bytes(),
        0xb304a621 => true.to_bytes(),
        0xb4352016 => tl::enums::stories::Stories::Stories(tl::types::stories::Stories {
            count: 0,
            stories: Vec::new(),
            pinned_to_top: None,
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xb43df344 => true.to_bytes(),
        0xb45ced1d => true.to_bytes(),
        0xb5052fea => true.to_bytes(),
        0xb550d328 => true.to_bytes(),
        0xb574b16b => true.to_bytes(),
        0xb60f5918 => tl::enums::users::UserFull::Full(tl::types::users::UserFull {
            full_user: tl::enums::UserFull::Full(tl::types::UserFull {
                blocked: false,
                phone_calls_available: false,
                phone_calls_private: false,
                can_pin_message: false,
                has_scheduled: false,
                video_calls_available: false,
                voice_messages_forbidden: false,
                translations_disabled: false,
                stories_pinned_available: false,
                blocked_my_stories_from: false,
                wallpaper_overridden: false,
                contact_require_premium: false,
                read_dates_private: false,
                sponsored_enabled: false,
                can_view_revenue: false,
                bot_can_manage_emoji_status: false,
                display_gifts_button: false,
                noforwards_my_enabled: false,
                noforwards_peer_enabled: false,
                unofficial_security_risk: false,
                id: 0,
                about: None,
                settings: tl::enums::PeerSettings::Settings(tl::types::PeerSettings {
                    report_spam: false,
                    add_contact: false,
                    block_contact: false,
                    share_contact: false,
                    need_contacts_exception: false,
                    report_geo: false,
                    autoarchived: false,
                    invite_members: false,
                    request_chat_broadcast: false,
                    business_bot_paused: false,
                    business_bot_can_reply: false,
                    geo_distance: None,
                    request_chat_title: None,
                    request_chat_date: None,
                    business_bot_id: None,
                    business_bot_manage_url: None,
                    charge_paid_message_stars: None,
                    registration_month: None,
                    phone_country: None,
                    name_change_date: None,
                    photo_change_date: None,
                }),
                personal_photo: None,
                profile_photo: None,
                fallback_photo: None,
                notify_settings: tl::enums::PeerNotifySettings::Settings(
                    tl::types::PeerNotifySettings {
                        show_previews: None,
                        silent: None,
                        mute_until: None,
                        ios_sound: None,
                        android_sound: None,
                        other_sound: None,
                        stories_muted: None,
                        stories_hide_sender: None,
                        stories_ios_sound: None,
                        stories_android_sound: None,
                        stories_other_sound: None,
                    },
                ),
                bot_info: None,
                pinned_msg_id: None,
                common_chats_count: 0,
                folder_id: None,
                ttl_period: None,
                theme: None,
                private_forward_name: None,
                bot_group_admin_rights: None,
                bot_broadcast_admin_rights: None,
                wallpaper: None,
                stories: None,
                business_work_hours: None,
                business_location: None,
                business_greeting_message: None,
                business_away_message: None,
                business_intro: None,
                birthday: None,
                personal_channel_id: None,
                personal_channel_message: None,
                stargifts_count: None,
                starref_program: None,
                bot_verification: None,
                send_paid_messages_stars: None,
                disallowed_gifts: None,
                stars_rating: None,
                stars_my_pending_rating: None,
                stars_my_pending_rating_date: None,
                main_tab: None,
                saved_music: None,
                note: None,
                bot_manager_id: None,
            }),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xb627f3aa => true.to_bytes(),
        0xb6c8c393 => tl::enums::contacts::SponsoredPeers::Empty.to_bytes(),
        0xb6e0a3f5 => tl::enums::stats::MessageStats::Stats(tl::types::stats::MessageStats {
            views_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            reactions_by_emotion_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
        })
        .to_bytes(),
        0xb7e085fe => tl::enums::auth::LoginToken::Token(tl::types::auth::LoginToken {
            expires: 0,
            token: Vec::new(),
        })
        .to_bytes(),
        0xb80e5fe4 => tl::enums::Updates::TooLong.to_bytes(),
        0xb81b93d4 => tl::enums::help::PremiumPromo::Promo(tl::types::help::PremiumPromo {
            status_text: String::new(),
            status_entities: Vec::new(),
            video_sections: Vec::new(),
            videos: Vec::new(),
            period_options: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xb86e380e => tl::enums::messages::VotesList::List(tl::types::messages::VotesList {
            count: 0,
            votes: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
            next_offset: None,
        })
        .to_bytes(),
        0xb880bc4b => true.to_bytes(),
        0xb8a0a1a8 => tl::enums::messages::AllStickers::NotModified.to_bytes(),
        0xb8f106e3 => tl::enums::InputBotInlineMessageId::Id(tl::types::InputBotInlineMessageId {
            dc_id: 0,
            id: 0,
            access_hash: 0,
        })
        .to_bytes(),
        0xb9b2881f => {
            tl::enums::stories::StoryReactionsList::List(tl::types::stories::StoryReactionsList {
                count: 0,
                reactions: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
                next_offset: None,
            })
            .to_bytes()
        }
        0xb9cdc5ee => Vec::<tl::enums::FactCheck>::new().to_bytes(),
        0xb9d9a38d => true.to_bytes(),
        0xb9ffc55b => true.to_bytes(),
        0xba4a3b5b => true.to_bytes(),
        0xba6705f0 => true.to_bytes(),
        0xbb12a419 => true.to_bytes(),
        0xbb3b9804 => true.to_bytes(),
        0xbb8125ba => tl::enums::messages::Reactions::NotModified.to_bytes(),
        0xbd0415c4 => true.to_bytes(),
        0xbd0d99eb => tl::enums::bots::ExportedBotToken::Token(tl::types::bots::ExportedBotToken {
            token: String::new(),
        })
        .to_bytes(),
        0xbd38850a => tl::enums::Updates::TooLong.to_bytes(),
        0xbd7f90ac => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0xbdbb0464 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0xbdca2f75 => tl::enums::messages::ExportedChatInvite::Invite(
            tl::types::messages::ExportedChatInvite {
                invite: tl::enums::ExportedChatInvite::ChatInviteExported(
                    tl::types::ChatInviteExported {
                        revoked: false,
                        permanent: false,
                        request_needed: false,
                        link: String::new(),
                        admin_id: 0,
                        date: 0,
                        start_date: None,
                        expire_date: None,
                        usage_limit: None,
                        usage: None,
                        requested: None,
                        subscription_expired: None,
                        title: None,
                        subscription_pricing: None,
                    },
                ),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0xbdf93428 => tl::enums::messages::Reactions::NotModified.to_bytes(),
        0xbe5335be => tl::enums::upload::File::File(tl::types::upload::File {
            r#type: tl::enums::storage::FileType::FileUnknown,
            mtime: 0,
            bytes: Vec::new(),
        })
        .to_bytes(),
        0xbe7e8ef1 => tl::enums::ResPq::Pq(tl::types::ResPq {
            nonce: [0u8; 16],
            server_nonce: [0u8; 16],
            pq: Vec::new(),
            server_public_key_fingerprints: Vec::new(),
        })
        .to_bytes(),
        0xbf25b7f3 => tl::enums::KeyboardButton::Button(tl::types::KeyboardButton {
            style: None,
            text: String::new(),
        })
        .to_bytes(),
        0xbf899aa0 => true.to_bytes(),
        0xc0111fe3 => tl::enums::Updates::TooLong.to_bytes(),
        0xc0977421 => {
            tl::enums::help::PromoData::Empty(tl::types::help::PromoDataEmpty { expires: 0 })
                .to_bytes()
        }
        0xc0cf7646 => tl::enums::Updates::TooLong.to_bytes(),
        0xc1cbd5b6 => true.to_bytes(),
        0xc2510192 => tl::enums::bots::PopularAppBots::Bots(tl::types::bots::PopularAppBots {
            next_offset: None,
            users: Vec::new(),
        })
        .to_bytes(),
        0xc27dfa68 => tl::enums::stats::PollStats::Stats(tl::types::stats::PollStats {
            votes_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
        })
        .to_bytes(),
        0xc4a353ee => Vec::<tl::enums::ContactStatus>::new().to_bytes(),
        0xc4f9186b => tl::enums::Config::Config(tl::types::Config {
            default_p2p_contacts: false,
            preload_featured_stickers: false,
            revoke_pm_inbox: false,
            blocked_mode: false,
            force_try_ipv6: false,
            date: 0,
            expires: 0,
            test_mode: false,
            this_dc: 0,
            dc_options: Vec::new(),
            dc_txt_domain_name: String::new(),
            chat_size_max: 0,
            megagroup_size_max: 0,
            forwarded_count_max: 0,
            online_update_period_ms: 0,
            offline_blur_timeout_ms: 0,
            offline_idle_timeout_ms: 0,
            online_cloud_timeout_ms: 0,
            notify_cloud_delay_ms: 0,
            notify_default_delay_ms: 0,
            push_chat_period_ms: 0,
            push_chat_limit: 0,
            edit_time_limit: 0,
            revoke_time_limit: 0,
            revoke_pm_time_limit: 0,
            rating_e_decay: 0,
            stickers_recent_limit: 0,
            channels_read_media_period: 0,
            tmp_sessions: None,
            call_receive_timeout_ms: 0,
            call_ring_timeout_ms: 0,
            call_connect_timeout_ms: 0,
            call_packet_timeout_ms: 0,
            me_url_prefix: String::new(),
            autoupdate_url_prefix: None,
            gif_search_username: None,
            venue_search_username: None,
            img_search_username: None,
            static_maps_provider: None,
            caption_length_max: 0,
            message_length_max: 0,
            webfile_dc_id: 0,
            suggested_lang_code: None,
            lang_pack_version: None,
            base_lang_pack_version: None,
            reactions_default: None,
            autologin_token: None,
        })
        .to_bytes(),
        0xc563c1e4 => true.to_bytes(),
        0xc5ba3d86 => true.to_bytes(),
        0xc661ad08 => tl::enums::help::PassportConfig::NotModified.to_bytes(),
        0xc727bb3b => true.to_bytes(),
        0xc78fe460 => tl::enums::messages::StickerSetInstallResult::Success.to_bytes(),
        0xc8a0ec74 => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        0xc9a47b0b => true.to_bytes(),
        0xc9e01e7b => tl::enums::WebViewResult::Url(tl::types::WebViewResultUrl {
            fullsize: false,
            fullscreen: false,
            same_origin: false,
            query_id: None,
            url: String::new(),
        })
        .to_bytes(),
        0xc9e33d54 => tl::enums::messages::InvitedUsers::Users(tl::types::messages::InvitedUsers {
            updates: tl::enums::Updates::TooLong,
            missing_invitees: Vec::new(),
        })
        .to_bytes(),
        0xc9f81ce8 => tl::enums::account::PrivacyRules::Rules(tl::types::account::PrivacyRules {
            rules: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xca8ae8ba => true.to_bytes(),
        0xcae47523 => tl::enums::auth::SentCode::Code(tl::types::auth::SentCode {
            r#type: tl::enums::auth::SentCodeType::App(tl::types::auth::SentCodeTypeApp {
                length: 0,
            }),
            phone_code_hash: String::new(),
            next_type: None,
            timeout: None,
        })
        .to_bytes(),
        0xcb9deff6 => true.to_bytes(),
        0xcbc6d107 => tl::enums::messages::InvitedUsers::Users(tl::types::messages::InvitedUsers {
            updates: tl::enums::Updates::TooLong,
            missing_invitees: Vec::new(),
        })
        .to_bytes(),
        0xcc104937 => true.to_bytes(),
        0xcc5b67cc => Vec::<tl::enums::StickerSetCovered>::new().to_bytes(),
        0xcc6e0c11 => true.to_bytes(),
        0xccfddf96 => true.to_bytes(),
        0xcd984aa5 => tl::enums::LangPackDifference::Difference(tl::types::LangPackDifference {
            lang_code: String::new(),
            from_version: 0,
            version: 0,
            strings: Vec::new(),
        })
        .to_bytes(),
        0xcdd42a05 => true.to_bytes(),
        0xce03da83 => {
            tl::enums::chatlists::ExportedInvites::Invites(tl::types::chatlists::ExportedInvites {
                invites: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0xcecc1134 => tl::enums::Updates::TooLong.to_bytes(),
        0xcf1592db => true.to_bytes(),
        0xcff43f61 => true.to_bytes(),
        0xd069ccde => tl::enums::Updates::TooLong.to_bytes(),
        0xd0b5e1fc => tl::enums::messages::MyStickers::Stickers(tl::types::messages::MyStickers {
            count: 0,
            sets: Vec::new(),
        })
        .to_bytes(),
        0xd1435160 => tl::enums::DestroyAuthKeyRes::DestroyAuthKeyNone.to_bytes(),
        0xd1810907 => tl::enums::stories::FoundStories::Stories(tl::types::stories::FoundStories {
            count: 0,
            stories: Vec::new(),
            next_offset: None,
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xd18b4d16 => {
            tl::enums::auth::Authorization::Authorization(tl::types::auth::Authorization {
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }),
            })
            .to_bytes()
        }
        0xd1da940c => tl::enums::Updates::TooLong.to_bytes(),
        0xd2816f10 => {
            tl::enums::messages::AffectedHistory::History(tl::types::messages::AffectedHistory {
                pts: 0,
                pts_count: 0,
                offset: 0,
            })
            .to_bytes()
        }
        0xd2aaf7ec => tl::enums::Updates::TooLong.to_bytes(),
        0xd30d78d4 => tl::enums::Updates::TooLong.to_bytes(),
        0xd348bc44 => tl::enums::Updates::TooLong.to_bytes(),
        0xd360e72c => tl::enums::help::SupportName::Name(tl::types::help::SupportName {
            name: String::new(),
        })
        .to_bytes(),
        0xd3e03124 => tl::enums::Updates::TooLong.to_bytes(),
        0xd464a42b => true.to_bytes(),
        0xd483f2a8 => tl::enums::messages::QuickReplies::NotModified.to_bytes(),
        0xd58f130a => true.to_bytes(),
        0xd5a5d3a1 => tl::enums::messages::Stickers::NotModified.to_bytes(),
        0xd5b10c26 => {
            tl::enums::EmojiUrl::Url(tl::types::EmojiUrl { url: String::new() }).to_bytes()
        }
        0xd638de89 => tl::enums::account::Themes::NotModified.to_bytes(),
        0xd63d94e0 => tl::enums::messages::SavedDialogs::NotModified(
            tl::types::messages::SavedDialogsNotModified { count: 0 },
        )
        .to_bytes(),
        0xd6753386 => tl::enums::account::EmojiStatuses::NotModified.to_bytes(),
        0xd69b8361 => true.to_bytes(),
        0xd6b94df2 => tl::enums::messages::PeerDialogs::Dialogs(tl::types::messages::PeerDialogs {
            dialogs: Vec::new(),
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
            state: tl::enums::updates::State::State(tl::types::updates::State {
                pts: 0,
                qts: 0,
                date: 0,
                seq: 0,
                unread_count: 0,
            }),
        })
        .to_bytes(),
        0xd712e4be => tl::enums::ServerDhParams::Fail(tl::types::ServerDhParamsFail {
            nonce: [0u8; 16],
            server_nonce: [0u8; 16],
            new_nonce_hash: [0u8; 16],
        })
        .to_bytes(),
        0xd897bc66 => {
            tl::enums::auth::PasswordRecovery::Recovery(tl::types::auth::PasswordRecovery {
                email_pattern: String::new(),
            })
            .to_bytes()
        }
        0xd89a83a3 => Vec::<tl::enums::RequirementToContact>::new().to_bytes(),
        0xd8aa3671 => tl::enums::Updates::TooLong.to_bytes(),
        0xd94305e0 => true.to_bytes(),
        0xd9ab0f54 => Vec::<tl::enums::Document>::new().to_bytes(),
        0xd9ba2e54 => tl::enums::Updates::TooLong.to_bytes(),
        0xda80f42f => tl::enums::help::PeerColors::NotModified.to_bytes(),
        0xdadbc950 => tl::enums::account::PrivacyRules::Rules(tl::types::account::PrivacyRules {
            rules: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xdaecc589 => tl::enums::messages::ComposedMessageWithAi::Ai(
            tl::types::messages::ComposedMessageWithAi {
                result_text: tl::enums::TextWithEntities::Entities(tl::types::TextWithEntities {
                    text: String::new(),
                    entities: Vec::new(),
                }),
                diff_text: None,
            },
        )
        .to_bytes(),
        0xdaeda864 => tl::enums::contacts::ContactBirthdays::Birthdays(
            tl::types::contacts::ContactBirthdays {
                contacts: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0xdb7e1747 => true.to_bytes(),
        0xdc0242c8 => tl::enums::Updates::TooLong.to_bytes(),
        0xdcd914fd => tl::enums::bots::BotInfo::Info(tl::types::bots::BotInfo {
            name: String::new(),
            about: String::new(),
            description: String::new(),
        })
        .to_bytes(),
        0xdcdf8607 => tl::enums::stats::MegagroupStats::Stats(tl::types::stats::MegagroupStats {
            period: tl::enums::StatsDateRangeDays::Days(tl::types::StatsDateRangeDays {
                min_date: 0,
                max_date: 0,
            }),
            members: tl::enums::StatsAbsValueAndPrev::Prev(tl::types::StatsAbsValueAndPrev {
                current: 0.0,
                previous: 0.0,
            }),
            messages: tl::enums::StatsAbsValueAndPrev::Prev(tl::types::StatsAbsValueAndPrev {
                current: 0.0,
                previous: 0.0,
            }),
            viewers: tl::enums::StatsAbsValueAndPrev::Prev(tl::types::StatsAbsValueAndPrev {
                current: 0.0,
                previous: 0.0,
            }),
            posters: tl::enums::StatsAbsValueAndPrev::Prev(tl::types::StatsAbsValueAndPrev {
                current: 0.0,
                previous: 0.0,
            }),
            growth_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            members_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            new_members_by_source_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            languages_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            messages_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            actions_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            top_hours_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            weekdays_graph: tl::enums::StatsGraph::Async(tl::types::StatsGraphAsync {
                token: String::new(),
            }),
            top_posters: Vec::new(),
            top_admins: Vec::new(),
            top_inviters: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xddbcd819 => true.to_bytes(),
        0xde7b673d => true.to_bytes(),
        0xde91436e => tl::enums::messages::ChatInviteJoinResult::Ok(
            tl::types::messages::ChatInviteJoinResultOk {
                updates: tl::enums::Updates::TooLong,
            },
        )
        .to_bytes(),
        0xdea20a39 => tl::enums::messages::AvailableEffects::NotModified.to_bytes(),
        0xdef60797 => true.to_bytes(),
        0xdf04dd4e => tl::enums::messages::ChatInviteImporters::Importers(
            tl::types::messages::ChatInviteImporters {
                count: 0,
                importers: Vec::new(),
                users: Vec::new(),
            },
        )
        .to_bytes(),
        0xdf77f3bc => true.to_bytes(),
        0xe085f4ea => tl::enums::Updates::TooLong.to_bytes(),
        0xe089f8f5 => tl::enums::Updates::TooLong.to_bytes(),
        0xe09d5faf => tl::enums::account::SavedMusicIds::NotModified.to_bytes(),
        0xe105e910 => tl::enums::Updates::TooLong.to_bytes(),
        0xe14c4a71 => tl::enums::photos::Photo::Photo(tl::types::photos::Photo {
            photo: tl::enums::Photo::Empty(tl::types::PhotoEmpty { id: 0 }),
            users: Vec::new(),
        })
        .to_bytes(),
        0xe1902288 => tl::enums::account::SavedRingtones::NotModified.to_bytes(),
        0xe2750328 => tl::enums::EmojiList::NotModified.to_bytes(),
        0xe320c158 => {
            tl::enums::account::Authorizations::Authorizations(tl::types::account::Authorizations {
                authorization_ttl_days: 0,
                authorizations: Vec::new(),
            })
            .to_bytes()
        }
        0xe34c0dd6 => Vec::<tl::enums::BotCommand>::new().to_bytes(),
        0xe39a8f03 => tl::enums::WallPaper::Paper(tl::types::WallPaper {
            id: 0,
            creator: false,
            default: false,
            pattern: false,
            dark: false,
            access_hash: 0,
            slug: String::new(),
            document: tl::enums::Document::Empty(tl::types::DocumentEmpty { id: 0 }),
            settings: None,
        })
        .to_bytes(),
        0xe3b7f82c => tl::enums::Updates::TooLong.to_bytes(),
        0xe40ca104 => {
            tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
                .to_bytes()
        }
        0xe42ce9c9 => tl::enums::account::ChatThemes::NotModified.to_bytes(),
        0xe470bcfd => tl::enums::messages::PeerDialogs::Dialogs(tl::types::messages::PeerDialogs {
            dialogs: Vec::new(),
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
            state: tl::enums::updates::State::State(tl::types::updates::State {
                pts: 0,
                qts: 0,
                date: 0,
                seq: 0,
                unread_count: 0,
            }),
        })
        .to_bytes(),
        0xe47cb579 => true.to_bytes(),
        0xe4cb9580 => tl::enums::Updates::TooLong.to_bytes(),
        0xe58e95d2 => {
            tl::enums::messages::AffectedMessages::Messages(tl::types::messages::AffectedMessages {
                pts: 0,
                pts_count: 0,
            })
            .to_bytes()
        }
        0xe5b17f2b => tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }).to_bytes(),
        0xe5bfffcd => tl::enums::auth::ExportedAuthorization::Authorization(
            tl::types::auth::ExportedAuthorization {
                id: 0,
                bytes: Vec::new(),
            },
        )
        .to_bytes(),
        0xe5f672fa => true.to_bytes(),
        0xe6213f4d => true.to_bytes(),
        0xe63fadeb => tl::enums::ExportedMessageLink::Link(tl::types::ExportedMessageLink {
            link: String::new(),
            html: String::new(),
        })
        .to_bytes(),
        0xe6df7378 => tl::enums::Updates::TooLong.to_bytes(),
        0xe71a4810 => true.to_bytes(),
        0xe7512126 => {
            tl::enums::DestroySessionRes::DestroySessionNone(tl::types::DestroySessionNone {
                session_id: 0,
            })
            .to_bytes()
        }
        0xe785a43f => tl::enums::channels::SendAsPeers::Peers(tl::types::channels::SendAsPeers {
            peers: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xe822649d => tl::enums::messages::HighScores::Scores(tl::types::messages::HighScores {
            scores: Vec::new(),
            users: Vec::new(),
        })
        .to_bytes(),
        0xe894ad4d => tl::enums::Authorization::Authorization(tl::types::Authorization {
            current: false,
            official_app: false,
            password_pending: false,
            encrypted_requests_disabled: false,
            call_requests_disabled: false,
            unconfirmed: false,
            hash: 0,
            device_model: String::new(),
            platform: String::new(),
            system_version: String::new(),
            api_id: 0,
            app_name: String::new(),
            app_version: String::new(),
            date_created: 0,
            date_active: 0,
            ip: String::new(),
            country: String::new(),
            region: String::new(),
        })
        .to_bytes(),
        0xea1f0c52 => tl::enums::account::Passkeys::Passkeys(tl::types::account::Passkeys {
            passkeys: Vec::new(),
        })
        .to_bytes(),
        0xea8ca4f9 => true.to_bytes(),
        0xeab5dc38 => true.to_bytes(),
        0xeabbb94c => tl::enums::Updates::TooLong.to_bytes(),
        0xeb2b4cf6 => {
            tl::enums::GlobalPrivacySettings::Settings(tl::types::GlobalPrivacySettings {
                archive_and_mute_new_noncontact_peers: false,
                keep_archived_unmuted: false,
                keep_archived_folders: false,
                hide_read_marks: false,
                new_noncontact_peers_require_premium: false,
                display_gifts_button: false,
                noncontact_peers_paid_stars: None,
                disallowed_gifts: None,
            })
            .to_bytes()
        }
        0xec22cfcd => true.to_bytes(),
        0xec86017a => true.to_bytes(),
        0xece2a0e6 => tl::enums::User::Empty(tl::types::UserEmpty { id: 0 }).to_bytes(),
        0xed9f30c5 => true.to_bytes(),
        0xeda3e33b => tl::enums::Updates::TooLong.to_bytes(),
        0xedd4882a => tl::enums::updates::State::State(tl::types::updates::State {
            pts: 0,
            qts: 0,
            date: 0,
            seq: 0,
            unread_count: 0,
        })
        .to_bytes(),
        0xedd49ef0 => tl::enums::Updates::TooLong.to_bytes(),
        0xee72f79a => true.to_bytes(),
        0xeeb0d625 => {
            tl::enums::stories::AllStories::NotModified(tl::types::stories::AllStoriesNotModified {
                state: String::new(),
                stealth_mode: tl::enums::StoriesStealthMode::Mode(tl::types::StoriesStealthMode {
                    active_until_date: None,
                    cooldown_until_date: None,
                }),
            })
            .to_bytes()
        }
        0xef500eab => true.to_bytes(),
        0xefd48c89 => {
            tl::enums::messages::DialogFilters::Filters(tl::types::messages::DialogFilters {
                tags_enabled: false,
                filters: Vec::new(),
            })
            .to_bytes()
        }
        0xefd9a6a2 => {
            tl::enums::messages::PeerSettings::Settings(tl::types::messages::PeerSettings {
                settings: tl::enums::PeerSettings::Settings(tl::types::PeerSettings {
                    report_spam: false,
                    add_contact: false,
                    block_contact: false,
                    share_contact: false,
                    need_contacts_exception: false,
                    report_geo: false,
                    autoarchived: false,
                    invite_members: false,
                    request_chat_broadcast: false,
                    business_bot_paused: false,
                    business_bot_can_reply: false,
                    geo_distance: None,
                    request_chat_title: None,
                    request_chat_date: None,
                    business_bot_id: None,
                    business_bot_manage_url: None,
                    charge_paid_message_stars: None,
                    registration_month: None,
                    phone_country: None,
                    name_change_date: None,
                    photo_change_date: None,
                }),
                chats: Vec::new(),
                users: Vec::new(),
            })
            .to_bytes()
        }
        0xefea3803 => Vec::<tl::enums::LangPackString>::new().to_bytes(),
        0xf0d3e6a8 => tl::enums::Updates::TooLong.to_bytes(),
        0xf107e790 => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0xf12e57c9 => tl::enums::Updates::TooLong.to_bytes(),
        0xf132e3ef => tl::enums::Updates::TooLong.to_bytes(),
        0xf1d0fbd3 => true.to_bytes(),
        0xf21f7f2f => tl::enums::messages::BotPreparedInlineMessage::Message(
            tl::types::messages::BotPreparedInlineMessage {
                id: String::new(),
                expire_date: 0,
            },
        )
        .to_bytes(),
        0xf257106c => true.to_bytes(),
        0xf2c4f24d => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0xf2f2330a => tl::enums::LangPackDifference::Difference(tl::types::LangPackDifference {
            lang_code: String::new(),
            from_version: 0,
            version: 0,
            strings: Vec::new(),
        })
        .to_bytes(),
        0xf3427b8c => tl::enums::Pong::Pong(tl::types::Pong {
            msg_id: 0,
            ping_id: 0,
        })
        .to_bytes(),
        0xf393aea0 => true.to_bytes(),
        0xf3ed4c73 => true.to_bytes(),
        0xf44a8315 => true.to_bytes(),
        0xf5045f1f => tl::enums::SetClientDhParamsAnswer::DhGenOk(tl::types::DhGenOk {
            nonce: [0u8; 16],
            server_nonce: [0u8; 16],
            new_nonce_hash1: [0u8; 16],
        })
        .to_bytes(),
        0xf50dbaa1 => true.to_bytes(),
        0xf516760b => {
            tl::enums::messages::Messages::NotModified(tl::types::messages::MessagesNotModified {
                count: 0,
            })
            .to_bytes()
        }
        0xf5537ebc => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        0xf5b5563f => true.to_bytes(),
        0xf5dad378 => {
            tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
                .to_bytes()
        }
        0xf64daf43 => {
            tl::enums::EncryptedChat::Empty(tl::types::EncryptedChatEmpty { id: 0 }).to_bytes()
        }
        0xf731a9f4 => true.to_bytes(),
        0xf743b857 => tl::enums::Updates::TooLong.to_bytes(),
        0xf7760f51 => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        0xf831a20f => tl::enums::Updates::TooLong.to_bytes(),
        0xf836aa95 => tl::enums::Updates::TooLong.to_bytes(),
        0xf8654027 => tl::enums::ExportedContactToken::Token(tl::types::ExportedContactToken {
            url: String::new(),
            expires: 0,
        })
        .to_bytes(),
        0xf8b036af => {
            tl::enums::messages::Chats::Chats(tl::types::messages::Chats { chats: Vec::new() })
                .to_bytes()
        }
        0xf96e55de => true.to_bytes(),
        0xf9cbe409 => tl::enums::messages::AffectedFoundMessages::Messages(
            tl::types::messages::AffectedFoundMessages {
                pts: 0,
                pts_count: 0,
                offset: 0,
                messages: Vec::new(),
            },
        )
        .to_bytes(),
        0xfa8cc6f5 => true.to_bytes(),
        0xfb7e8ca7 => tl::enums::messages::EmojiGameInfo::EmojiGameUnavailable.to_bytes(),
        0xfbd3de6b => true.to_bytes(),
        0xfbfca18f => tl::enums::messages::AllStickers::NotModified.to_bytes(),
        0xfc533372 => tl::enums::Updates::TooLong.to_bytes(),
        0xfc78af9b => tl::enums::ReportResult::ChooseOption(tl::types::ReportResultChooseOption {
            title: String::new(),
            options: Vec::new(),
        })
        .to_bytes(),
        0xfc8ddbea => tl::enums::WallPaper::Paper(tl::types::WallPaper {
            id: 0,
            creator: false,
            default: false,
            pattern: false,
            dark: false,
            access_hash: 0,
            slug: String::new(),
            document: tl::enums::Document::Empty(tl::types::DocumentEmpty { id: 0 }),
            settings: None,
        })
        .to_bytes(),
        0xfd2dda49 => true.to_bytes(),
        0xfda68d36 => {
            tl::enums::messages::MessageEditData::Data(tl::types::messages::MessageEditData {
                caption: false,
            })
            .to_bytes()
        }
        0xfdbcd714 => Vec::<tl::enums::Peer>::new().to_bytes(),
        0xfe2eda76 => true.to_bytes(),
        0xfeed5769 => true.to_bytes(),
        0xfef48f62 => tl::enums::Updates::TooLong.to_bytes(),
        0xffb6d4ca => tl::enums::messages::StickerSet::NotModified.to_bytes(),
        _ => return None,
    };
    Some(body)
}
