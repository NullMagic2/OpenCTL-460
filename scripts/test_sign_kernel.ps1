# Creates a free, self-signed development package. Does not modify certificate stores or boot settings.
# A temporary password-protected private key stays in debug/generated and is deleted after signing.
param([string]$OsslSignCode)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$source = Join-Path $projectRoot 'dist\kernel'
$destination = Join-Path $projectRoot 'dist\kernel-test'
$generated = Join-Path $projectRoot 'debug\generated'
foreach ($file in @('ctl460_vhf.sys','ctl460_vhf.inf')) {if (-not (Test-Path -LiteralPath (Join-Path $source $file))) {throw 'Run build_kernel_driver.bat first.'}}
$kitRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$signTool = Get-ChildItem -LiteralPath $kitRoot -Recurse -Filter signtool.exe | Where-Object FullName -Match '\\x64\\' | Sort-Object FullName -Descending | Select-Object -First 1
$catalogTool = Get-ChildItem -LiteralPath $kitRoot -Recurse -Filter Inf2Cat.exe | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signTool -or -not $catalogTool) {throw 'Install the Windows SDK/WDK signing tools.'}
New-Item -ItemType Directory -Force -Path $destination,$generated | Out-Null
Copy-Item -LiteralPath (Join-Path $source 'ctl460_vhf.sys'),(Join-Path $source 'ctl460_vhf.inf') -Destination $destination -Force
$rsa = [Security.Cryptography.RSA]::Create(2048)
$request = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=CTL460 Studio Development Test', $rsa, [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pkcs1)
$usage = [Security.Cryptography.OidCollection]::new()
[void]$usage.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'))
$request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($usage,$false))
$request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new([Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature,$true))
$certificate = $request.CreateSelfSigned([DateTimeOffset]::UtcNow.AddMinutes(-5),[DateTimeOffset]::UtcNow.AddYears(1))
$password = [Guid]::NewGuid().ToString('N') + [Guid]::NewGuid().ToString('N')
$keyPath = Join-Path $generated ('test-sign-' + [Guid]::NewGuid().ToString('N') + '.pfx')
try {
    [IO.File]::WriteAllBytes($keyPath,$certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx,$password))
    [IO.File]::WriteAllBytes((Join-Path $destination 'CTL460-Test.cer'),$certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Cert))
    # OpenSSL signing keeps key handling inside the build folder on restricted build hosts.
    # Both paths produce ordinary Authenticode signatures; Windows trust policy is unchanged.
    if($OsslSignCode) {
        Remove-Item -LiteralPath (Join-Path $destination 'ctl460_vhf.sys') -Force
        & $OsslSignCode sign -h sha256 -pkcs12 $keyPath -pass $password -in (Join-Path $source 'ctl460_vhf.sys') -out (Join-Path $destination 'ctl460_vhf.sys')
    } else {
        & $signTool.FullName sign /fd SHA256 /f $keyPath /p $password (Join-Path $destination 'ctl460_vhf.sys')
    }
    if ($LASTEXITCODE -ne 0) {throw 'Embedded test signing failed'}
    & $catalogTool.FullName ('/driver:' + $destination) /os:10_X64 /uselocaltime
    if ($LASTEXITCODE -ne 0) {throw 'Test catalog generation failed'}
    if($OsslSignCode) {
        $unsignedCatalog=Join-Path $destination 'ctl460_vhf.unsigned.cat'
        Move-Item -LiteralPath (Join-Path $destination 'ctl460_vhf.cat') -Destination $unsignedCatalog -Force
        & $OsslSignCode sign -h sha256 -pkcs12 $keyPath -pass $password -in $unsignedCatalog -out (Join-Path $destination 'ctl460_vhf.cat')
    } else {
        & $signTool.FullName sign /fd SHA256 /f $keyPath /p $password (Join-Path $destination 'ctl460_vhf.cat')
    }
    if ($LASTEXITCODE -ne 0) {throw 'Catalog test signing failed'}
    if($OsslSignCode) {Remove-Item -LiteralPath $unsignedCatalog -Force}
    $instructions = "Development-only self-signed virtual HID package.`r`nCertificate thumbprint: $($certificate.Thumbprint)`r`nSee docs/SIGNING.md for setup and removal. This script changed no trust store or boot setting.`r`n"
    [IO.File]::WriteAllText((Join-Path $destination 'README.txt'),$instructions)
    & (Join-Path $PSScriptRoot 'package_hid_setup.ps1')
    Write-Output 'Free test-signed package created in dist/kernel-test. No private key is distributed.'
} finally {
    # Delete exactly the temporary key we created inside the verified debug directory.
    $resolvedKey = [IO.Path]::GetFullPath($keyPath)
    if (-not $resolvedKey.StartsWith([IO.Path]::GetFullPath($generated).TrimEnd('\')+'\',[StringComparison]::OrdinalIgnoreCase)) {throw 'Temporary key path escaped debug directory'}
    if (Test-Path -LiteralPath $resolvedKey) {Remove-Item -LiteralPath $resolvedKey -Force}
    $certificate.Dispose(); $rsa.Dispose(); $password = $null
}
