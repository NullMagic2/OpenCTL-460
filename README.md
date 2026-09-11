# OpenCTL 460

A modern Windows driver and configuration utility for the **Wacom Bamboo CTL-460**, written from the ground up in **Rust**.

OpenCTL 460 is aimed at keeping the CTL-460 useful on modern Windows systems while improving responsiveness, smoothing, pressure handling, and application compatibility beyond what the legacy official driver offers.

*Note: Some features, especially assisted handwriting and virtual HID operation, may still be under development.*

---

## Highlights

- **Entirely new Rust driver**  
  A clean, modern implementation rather than a wrapper around the legacy Wacom driver.

- **Wintab and Windows Ink at the same time**  
  Choose the input API **per application**, so older drawing software can use Wintab while newer applications use Windows Ink without constantly changing the global driver configuration.

- **Lower-latency, more responsive input**  
  Designed to reduce the heavy or delayed feel of the official legacy driver.

- **Smoother pen response**  
  Configurable smoothing, stabilization, wobble reduction, and motion filtering help keep lines steady without forcing a single drawing feel on every user.

- **User-space or Virtual HID operation**  
  Run in user space for straightforward testing and development, or through the **Virtual HID** path when using Windows test mode.

- **Pressure curves and presets**  
  Create, edit, save, and switch between pressure-response curves for different brushes, applications, or drawing styles.

- **Assisted handwriting mode — experimental**  
  Optional handwriting-oriented assistance can adjust smoothing behavior for writing while preserving a more natural drawing response.

  

---

## Pen pressure and curve presets

OpenCTL 460 provides direct control over tip sensitivity, double-click distance, and the pen's pressure response. Pressure curves can be edited visually and saved as reusable presets.


<img width="2048" height="1852" alt="image" src="https://github.com/user-attachments/assets/f837c74b-c85e-449f-96db-d2d9f53b0d2f" />


The goal is to let the physical **1024 pressure levels** of the CTL-460 map naturally into a higher-resolution software response, while still allowing users to tune the feel to their hand and brush setup.

---

## Pen button configuration

Both pen switches can be configured independently, including mouse actions and keyboard shortcuts.

<img width="2048" height="1862" alt="image" src="https://github.com/user-attachments/assets/9ac56b5e-070a-42de-8a0a-1855aeb81994" />

This makes it possible to keep common drawing actions directly on the pen without relying on application-specific tablet settings.

---

## Smoothing and handwriting assistance

Rather than merely replicating the original driver behavior, this driver has improved stabilization controls:

- StreamLine path smoothing
- Pressure smoothing
- Speed-dependent stabilization
- Motion filtering
- Wobble reduction
- Circle and curve assistance
- Adaptive base smoothing
- Experimental handwriting detection / assistance

<img width="2048" height="1852" alt="image" src="https://github.com/user-attachments/assets/1f2e97df-4bc2-4e82-a273-1399f22a9f17" />


The intention is not to make every stroke artificially smooth. The controls are there so you can trade off **raw immediacy**, **stability**, and **handwriting assistance** according to the application and task.

---

## Wintab + Windows Ink, per application

One of OpenCTL 460's main goals is to avoid the usual all-or-nothing choice between legacy **Wintab** and **Windows Ink**.

Different applications can require different tablet APIs. With per-application configuration, a typical setup can look like this:

| Application type | Input API |
| --- | --- |
| Legacy art software | Wintab |
| Modern Windows drawing apps | Windows Ink |
| Handwriting / note-taking apps | Windows Ink |
| Software with better legacy tablet support | Wintab |

Both APIs can therefore coexist in the same driver setup instead of requiring repeated global changes.

---

## Driver modes

OpenCTL 460 supports two operating approaches:

### User-space mode

Useful for normal development, testing, and scenarios where a kernel-style virtual device is unnecessary.

### Virtual HID mode

Presents tablet input through a virtual HID path. This mode is intended for configurations where the operating system or application benefits from a HID-level device interface and may require **Windows test mode** during development.

---

## Why OpenCTL 460?

The Bamboo CTL-460 is still a capable drawing tablet, but its original software stack was designed for a very different generation of Windows.

OpenCTL 460 focuses on preserving the hardware while modernizing the software around it:

- Modern Rust codebase;
- Responsive input processing;
- Configurable smoothing instead of fixed filtering;
- Simultanoues Modern Windows Ink and Wintab support (no restart or reboot needed!);
- Flexible per-application behavior;
- Pressure-curve presets;
- Experimental handwriting assistance;
- A clean Windows configuration interface.

---
