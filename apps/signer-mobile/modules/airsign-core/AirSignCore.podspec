Pod::Spec.new do |s|
  s.name           = 'AirSignCore'
  s.version        = '1.0.0'
  s.summary        = 'AirSign cryptographic core — WKWebView/WASM bridge (v2)'
  s.description    = <<-DESC
    Expo native module for AirSign cryptographic operations.
    v2: Swift WKWebView bridge loading afterimage-wasm via airsign_bridge.html.
        Ed25519 signing is performed inside WASM linear memory.
        Private keys are stored in the iOS Keychain with
        kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly.
  DESC
  s.homepage       = 'https://github.com/nzengi/AirSign'
  s.license        = { :type => 'MIT' }
  s.authors        = { 'AirSign' => 'nzengi@github.com' }
  s.platform       = :ios, '15.1'
  s.swift_version  = '5.4'
  s.source         = { :path => '.' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  s.source_files   = 'src/**/*.{swift,h,m}'

  # Bundle the bridge HTML and pre-built WASM artifacts.
  # Accessible at runtime via Bundle.main.url(forResource:withExtension:subdirectory:)
  # using subdirectory "AirSignCore_assets".
  s.resources      = 'assets/**/*'
  s.resource_bundles = {
    'AirSignCore_assets' => ['assets/**/*']
  }

  s.frameworks     = 'WebKit', 'Security'
end
