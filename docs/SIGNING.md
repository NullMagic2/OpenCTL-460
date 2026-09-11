<!-- Explains free driver deployment, what Windows settings change, and how to undo them. -->
# Free installation and kernel driver signing

You do not need to buy a certificate to develop and use this project on a PC.

The normal CTL-460 Studio installer installs the user-mode Windows Ink feeder, settings app,
and both WinTab DLLs. These components do not require Microsoft kernel-driver signing.
They use the pressure, handwriting and simulated-tilt processing in this project. Applications
that refuse synthetic pen input can require the virtual HID backend instead.

## Free virtual HID development installation

### Installation and routine startup

The installer uses its one administrator approval to trust the bundled development
certificate, install the kernel package, and register the OpenCTL 460 service.
Windows signing prerequisites below must already be satisfied. Setup does not
change Test Mode, Secure Boot, Memory Integrity or firmware settings.

After installation, select Virtual HID (installed service). The feeder runs as your
normal user and talks to the service without UAC. The service owns the administrator-only
kernel handle and temporarily pauses/restores the two known Wacom services.
See [SERVICE.md](SERVICE.md) for the security boundary and verification limits.

Newly added certificate trust is rolled back if kernel package installation fails.
Successful trust entries remain until removed using the instructions below.
### Manual setup, if needed

This path installs the experimental virtual HID driver on your PC using a self-signed
certificate and Windows Test Mode. It is a supported Windows development mechanism.
The steps below explain the required Windows settings and how to restore them.

1. If you used the supplied installer, the prebuilt package is already in
   `C:\Program Files\CTL460Studio\kernel-test` and the helper is in the parent directory.
   Use that parent directory wherever the commands below say `dist`; no rebuild is needed.
   To build from source instead, run `build_driver.bat`, then `build_kernel_driver.bat`, then `test_sign_driver.bat`.
   The last script produces `dist\kernel-test` with an embedded-signed `.sys`, signed catalog,
   INF, public certificate and README. It does not modify Windows trust or boot settings.
   The temporary private key is deleted after signing; every rebuild generates a new certificate.
2. Before changing firmware or boot settings on an encrypted machine, make sure you can access
   its BitLocker/device-encryption recovery key. Follow the machine vendor's firmware instructions.
   If Windows says the Test Mode setting is protected by Secure Boot policy, disable **Secure Boot**
   on this PC. Do not disable TPM or clear its keys.
3. Open **Command Prompt as administrator** on the PC, run this command, and restart:

   ```bat
   rem Enable loading of developer-signed kernel code on the PC.
   bcdedit /set testsigning on
   ```

4. After restarting, copy the complete `dist\kernel-test` directory and `dist\ctl460-setup.exe`
   to the PC. Check that the certificate thumbprint matches `kernel-test\README.txt`.
   From an administrator Command Prompt in the copied `dist` directory, run:

   ```bat
   rem Trust only the matching development certificate on this PC.
   certutil -addstore -f Root kernel-test\CTL460-Test.cer
   certutil -addstore -f TrustedPublisher kernel-test\CTL460-Test.cer
   rem Create our root virtual device and ask Windows to install the test-signed package.
   ctl460-setup.exe install kernel-test\ctl460_vhf.inf
   ```

5. Open CTL-460 Studio, select **Virtual HID (automatic setup)**,
   select the physical CTL-460 collection and start the feeder. This development kernel device
   accepts writes only from administrators/SYSTEM; Studio requests elevation for its supervisor.
   The WinTab provider reads the same processed
   stream independently. Restart if the installation helper reports that Windows requires it.
6. Follow `debug\HARDWARE-CHECKLIST.md` before using the driver for real work. Test Mode permits
   loading the driver; it does not establish that the driver is correct, safe or compatible.

Memory Integrity/HVCI does not automatically need to be disabled for this approach: Microsoft
requires the binary itself to be test-signed when HVCI is enabled. Our signing script signs
both the SYS and the catalog. HVCI compatibility still needs testing. Managed-device policies
can prohibit this workflow. Do not use `nointegritychecks`, patched loaders or vulnerable drivers.

## Undo the development installation

Stop the feeder and close tablet applications. In an administrator Command Prompt:

```bat
rem Remove only CTL-460 Studio's virtual device, then leave Test Mode.
ctl460-setup.exe remove
bcdedit /set testsigning off
```

Remove the matching certificate from both Local Computer stores using the exact thumbprint
recorded in the package README (replace `THUMBPRINT` below):

```bat
rem Remove only this package's development trust entries.
certutil -delstore Root THUMBPRINT
certutil -delstore TrustedPublisher THUMBPRINT
```

Restart and restore Secure Boot if you disabled it. Other development drivers that require
Test Mode will also stop loading. To remove the staged driver package from the Driver Store,
use `pnputil /enum-drivers`, identify **CTL460Vhf / ctl460_vhf.inf / CTL-460 Rust contributors**,
and delete only that matching published `oemNN.inf` with `pnputil /delete-driver oemNN.inf`.
The device-removal helper intentionally does not guess a Driver Store package name.

## Why not just disable signature enforcement once?

### Code 31 / 0xC0200209 after installation

This is `STATUS_WDF_OBJECT_ATTRIBUTES_INVALID`, an initialization failure in versions
through 0.1.6. Version 0.1.7 sets both child-object execution and synchronization defaults
as required by [WDF_OBJECT_ATTRIBUTES_INIT](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdfobject/nf-wdfobject-wdf_object_attributes_init).
Disabling Secure Boot again cannot correct these object attributes. Update the app and
click **Start driver** with Virtual HID selected, approving the setup prompt for the
new package certificate. Run `ctl460-setup status` to distinguish a failed device from
an uninstalled device. The 0.1.7 package has a higher INF driver version so Windows selects
it over the faulty package; the physical Wacom device is not rebound.

The fix was compiled and package-validated; successful loading still needs verification
after the updated package is installed on Windows.

### One-startup signature enforcement option

Windows' Advanced Startup option disables enforcement for that startup session. It does not
make an unsigned package a trusted driver and is not a durable installation method after reboot.
The documented Test Mode workflow is repeatable, and keeps each development binary signed.

A new custom kernel driver cannot normally load on Windows 11 with its normal signing policy
and Secure Boot intact just because its installer is an administrator. Normal distribution needs
Microsoft signing. The commonly encountered cost is the EV certificate needed for a Hardware
Dev Center account; using Test Mode avoids that purchase for development.

Official references, checked September 2026:

- [Microsoft: loading test-signed code and Secure Boot/HVCI requirements](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/the-testsigning-boot-configuration-option)
- [Microsoft: installing test-signed packages](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/installing-test-signed-driver-packages)
- [Microsoft: kernel driver signing policy](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-policy--windows-vista-and-later-)
- [Microsoft: Hardware Dev Center certificate requirements](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/code-signing-reqs)

