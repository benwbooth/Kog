# Rust exports Java_org_kog_player_NativeAudio_* entry points by name.
# Keep this small bridge stable while R8 optimizes the rest of the app.
-keep,allowoptimization class org.kog.player.NativeAudio { *; }
