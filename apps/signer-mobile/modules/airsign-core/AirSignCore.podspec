Pod::Spec.new do |s|
  s.name           = 'AirSignCore'
  s.version        = '1.0.0'
  s.summary        = 'AirSign cryptographic core — iOS native module skeleton (JSI bridge v2 upgrade path)'
  s.description    = <<-DESC
    Expo native module for AirSign cryptographic operations.
    v1: All crypto is handled by the TypeScript layer (tweetnacl + expo-secure-store).
    v2: This Swift layer will bridge to the afterimage-wasm binary via JavaScriptCore,
        providing direct Secure Enclave access and eliminating the JS thread hop.
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
end