fn main() {
    // Set macOS deployment target and C++ flags to fix whisper-rs filesystem issues
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.15");
        std::env::set_var("MACOSX_DEPLOYMENT_TARGET", "10.15");
        
        // Set C++ flags to try to work around filesystem issues
        std::env::set_var("CXXFLAGS", "-std=c++17 -mmacosx-version-min=10.15 -D_LIBCPP_DISABLE_AVAILABILITY");
        std::env::set_var("CPPFLAGS", "-mmacosx-version-min=10.15 -D_LIBCPP_DISABLE_AVAILABILITY");
        println!("cargo:rustc-env=CXXFLAGS=-std=c++17 -mmacosx-version-min=10.15 -D_LIBCPP_DISABLE_AVAILABILITY");
        println!("cargo:rustc-env=CPPFLAGS=-mmacosx-version-min=10.15 -D_LIBCPP_DISABLE_AVAILABILITY");
    }
    
    tauri_build::build()
}
