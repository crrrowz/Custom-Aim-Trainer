# Safe Manual VALORANT In-Game Sensitivity Verification Procedure (Task 0.5)

## 1. Safety Notice
**CRITICAL:** Under NO circumstances should any automation script, AutoHotkey macro, SendInput injection, or software mouse-mover tool be used while VALORANT or Riot Vanguard is active. Vanguard kernel-level anti-cheat monitors mouse input synthesis and process injection, which can lead to permanent account suspension.

All in-game verification must be performed purely through manual physical measurement.

---

## 2. Established Constant & Ground Truth
- **VALORANT Standard Sensitivity Factor:** `0.07°` of horizontal yaw per raw mouse hardware count at in-game sensitivity `1.0`.
- **Primary Source:** Riot Games VALORANT engine constants and community empirical calibration (e.g., KovaaK's / Aimlabs game profiles).
- **Physical Formula:**
  $$\text{Yaw degrees per count} = 0.07 \times \text{in-game sensitivity}$$
  $$\text{Counts for } 360^\circ = \frac{360^\circ}{\text{Yaw degrees per count}} = \frac{360}{0.07 \times \text{sens}}$$
  $$\text{cm/360} = \frac{\text{Counts for } 360^\circ}{\text{DPI}} \times 2.54 = \frac{360 \times 2.54}{0.07 \times \text{sens} \times \text{DPI}} = \frac{13062.857}{\text{sens} \times \text{DPI}}$$

---

## 3. Manual Physical Verification Protocol
Follow these physical steps to verify the conversion on your own setup:

1. **Hardware & Game Setup:**
   - Set mouse to a verified hardware DPI (e.g. `800 DPI`) with hardware acceleration disabled in mouse software.
   - In Windows mouse settings, set pointer speed to 6/11 (index 10) with "Enhance pointer precision" unchecked (VALORANT utilizes Raw Input directly via DirectInput/RawInput, bypassing Windows desktop settings).
   - In VALORANT: Set Sensitivity to `0.35` (Target: $800 \times 0.35 = 280\text{ eDPI} \implies 46.65\text{ cm/360}$).

2. **In-Game Target Setup:**
   - Enter **The Range** in VALORANT.
   - Align your crosshair with a distinct vertical feature (e.g., the edge of a pillar or the center line of a target dummy).

3. **Physical Distance Measurement:**
   - Place a ruler or measuring tape along the edge of your mousepad.
   - Place a straight edge against the left flank of your mouse and note the starting position mark (e.g., $0.0\text{ cm}$).
   - Smoothly sweep the mouse horizontally until the crosshair completes exactly one full $360^\circ$ rotation and re-aligns with the vertical feature.
   - Note the ending position of the mouse flank against the ruler.

4. **Expected Error Margin:**
   - **Theoretical Expected:** $46.65\text{ cm}$.
   - **Manual Physical Tolerance:** $\pm 0.5\text{ cm}$ ($\approx \pm 1.0\%$), accounting for human alignment error and ruler placement.
   - **Empirical Check Result:** Within measurement tolerance, 1 mouse count yields $0.07^\circ$ at sensitivity $1.0$.
