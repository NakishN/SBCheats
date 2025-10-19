@echo off
set RUST_LOG=warn
set EFRAME_LOG_LEVEL=warn
set EGUI_LOG_LEVEL=warn
set RUST_BACKTRACE=0
target\release\cheat_detector.exe
