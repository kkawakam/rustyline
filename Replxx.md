Replxx
```
.
├── ConvertUTF.cpp (no need)
├── ConvertUTF.h (idem)
├── conversion.cxx (no need)
├── conversion.hxx (idem)
├── escape.cxx (vs rustyline::tty::unix::PosixRawReader::escape_sequence)
├── escape.hxx (idem)
├── history.cxx
├── history.hxx
├── killring.hxx
├── prompt.cxx
├── prompt.hxx
├── replxx.cxx
├── replxx_impl.cxx
├── replxx_impl.hxx
├── terminal.cxx
├── terminal.hxx
├── unicodestring.hxx (vs unicode-segmentation)
├── utf8string.hxx (no need)
├── util.cxx
├── util.hxx
├── wcwidth.cpp (vs unicode-width)
├── windows.cxx
└── windows.hxx
```

-------------------------------------------------------------------------------------
### include/replxx.hxx

Replxx::input => rustyline::Editor::readline
Replxx::set_preload_buffer => rustyline::Editor::readline_with_initial
Replxx::clear_screen => ~ rustyline::tty::Renderer::clear_screen
Replxx::install_window_change_handler => ~ rustyline::tty::unix::install_sigwinch_handler
Replxx::print => _
Replxx::write => _
Replxx::set_prompt => _
Replxx::emulate_key_press => _

Replxx::Color => _

Replxx::State => rustyline::line_buffer::LineBuffer
Replxx::get_state => _
Replxx::set_state => ~ rustyline::completion::Completer::update

Replxx::KEY => rustyline::keys::KeyEvent
Replxx::ACTION => ~ rustyline::keymap::Cmd
Replxx::ACTION_RESULT => _
Replxx::key_press_handler_t => ~ rustyline::binding::EventHandler
Replxx::invoke => _
Replxx::bind_key => rustyline::Editor::bind_sequence
Replxx::bind_key_internal => ?

Replxx::Completion => rustyline::completion::Candidate + rustyline::highlight::Highlighter::highlight_candidate
Replxx::completion_callback_t => rustyline::completion::Completer
Replxx::set_completion_callback => rustyline::Editor::set_helper
Replxx::set_completion_count_cutoff => rustyline::config::Config::completion_prompt_limit
Replxx::set_double_tab_completion => _
Replxx::set_complete_on_empty => _
Replxx::set_beep_on_ambiguous_completion => rustyline::config::Config::bell_style
Replxx::set_immediate_completion => _

Replxx::highlighter_callback_t => rustyline::highlight::Highlighter::highlight
Replxx::set_highlighter_callback => rustyline::Editor::set_helper
Replxx::set_no_color => rustyline::config::Config::color_mode

Replxx::hint_callback_t => rustyline::hint::Hinter::hint
Replxx::set_hint_callback => rustyline::Editor::set_helper
Replxx::set_max_hint_rows => _
Replxx::set_hint_delay => _

Replxx::HistoryEntry => _
Replxx::HistoryScan => rustyline::history::Iter
Replxx::set_ignore_case => case_insensitive_history_search feature but nothing for completion
Replxx::set_unique_history => rustyline::config::Config::history_duplicates
Replxx::set_max_history_size => rustyline::config::Config::max_history_size
Replxx::history_add => rustyline::Editor::add_history_entry
Replxx::history_sync => _
Replxx::history_save => rustyline::Editor::save_history
Replxx::history_load => rustyline::Editor::load_history
Replxx::history_clear => rustyline::Editor::clear_history
Replxx::history_size => rustyline::history::History::len
Replxx::history_scan => rustyline::history::History::iter

Replxx::modify_callback_t => _
Replxx::set_modify_callback => _

Replxx::set_word_break_characters => _
Replxx::set_indent_multiline => _
Replxx::enable_bracketed_paste => rustyline::config::Config::enable_bracketed_paste
Replxx::disable_bracketed_paste => rustyline::config::Config::enable_bracketed_paste

-------------------------------------------------------------------------------------
### src/killring.hxx

replxx::KillRing => rustyline::kill_ring::KillRing
KillRing::action => rustyline::kill_ring::Action
KillRing::kill => KillRing::kill
KillRing::yank => KillRing::yank
KillRing::yankPop => KillRing::yank_pop
_ => KillRing::reset

-------------------------------------------------------------------------------------
### history.hxx

replxx::History::Entry => _
replxx::History => rustyline::history::History

History::add => History::add
History::save => History::save
History::load => History::load
History::clear => History::clear
History::is_empty => History::is_empty
History::size => History::size

History::set_max_size => History::set_max_len
History::set_unique => History::with_config / history_duplicates

History::reset_yank_iterator => _
History::next_yank_position => _
History::yank_line => _

History::reset_recall_most_recent => _
History::commit_index => _
History::update_last => _
History::drop_last => _
History::is_last => _
History::move => _
History::jump => _

History::current => _
History::set_current_scratch => _
History::reset_current_scratch => _
History::reset_scratches => _

History::common_prefix_search => History::starts_with
History::scan => History::iter

History::save_pos => _
History::restore_pos => _

-------------------------------------------------------------------------------------
### terminal.hxx

replxx::Terminal => rustyline::tty::Term
replxx::Terminal::EVENT_TYPE => ~ Result<KeyEvent>
replxx::Terminal::CLEAR_SCREEN => _

Terminal::write32 => rustyline::tty::Renderer::write_and_flush
Terminal::write8 => rustyline::tty::Renderer::write_and_flush
Terminal::get_screen_columns => rustyline::tty::Renderer::get_columns
Terminal::get_screen_rows => rustyline::tty::Renderer::get_rows
Terminal::enable_bracketed_paste => rustyline::tty::Term::new
Terminal::disable_bracketed_paste => rustyline::tty::Term::new
Terminal::enable_raw_mode => rustyline::tty::Term::enable_raw_mode
Terminal::reset_raw_mode => ?
Terminal::disable_raw_mode => rustyline::tty::RawMode::disable_raw_mode
Terminal::read_char => ? rustyline::tty::RawReader::next_key
Terminal::clear_screen => rustyline::tty::Renderer::clear_screen
Terminal::wait_for_input => rustyline::tty::RawReader::poll / rustyline::tty::RawReader::next_key
Terminal::notify_event => ?
Terminal::jump_cursor => rustyline::tty::Renderer::move_cursor
Terminal::set_cursor_visible => _
Terminal::read_verbatim => ~ rustyline::tty::RawReader::next_char
Terminal::install_window_change_handler => ~ rustyline::tty::unix::install_sigwinch_handler

| Rustyline | Replxx |
|-----------|--------|
|           |        |
