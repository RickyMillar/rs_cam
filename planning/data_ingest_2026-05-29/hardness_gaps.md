# Hardness Data — GAPS — 2026-05-29

Sources that were login-walled, bot-blocked, unreachable, or have no citeable value.
Per the overriding rule, NO value was invented to fill these; they are logged as gaps.

---

## Bot-blocked (HTTP 403)

| Source | URL | Reason | Workaround used |
|---|---|---|---|
| MatWeb (general datasheets, non-ASM) | https://www.matweb.com/search/DataSheet.aspx?MatGUID=... | HTTP 403 to scripted fetch; full datasheet values also gated behind free login | Used named supplier datasheets (Direct Plastics, Treatstock, Alro) + MakeItFrom + ASM mirror instead |
| MatWeb (acrylic datasheet) | https://www.matweb.com/search/datasheet.aspx?matguid=a8c4a51c41be4e0ba84f2538ed47b7e3 | HTTP 403 Forbidden | Acrylic Rockwell M93 obtained from MakeItFrom PMMA instead |
| Curbell Plastics (acrylic machining/material page) | https://www.curbellplastics.com/materials/plastics/acrylic/ | HTTP 403 Forbidden (bot-protected, as flagged in acquisition doc §C2) | Not needed for hardness; Curbell is machining-behavior (Grade B/C), not a hardness primary |
| Interstate Plastics (acrylic sheet) | https://www.interstateplastics.com/Acrylic-Sheet-ACRCLR.php | HTTP 403 Forbidden (bot-protected) | Not needed for hardness primary |

## SSL / certificate issues (worked around, not a data gap)

| Source | URL | Reason | Resolution |
|---|---|---|---|
| ASM / MatWeb aluminum data sheets | https://asm.matweb.com/search/SpecificMaterial.asp?bassnum=ma6061t6 (and ma7075t6) | WebFetch + plain curl failed with "unable to verify the first certificate" (incomplete intermediate-CA chain served by host) | Fetched via `curl -k`; content is the standard public ASM data sheet. Brinell values captured with verbatim quotes in `hardness.md` §3. **Not a data gap** — value obtained — but the cert chain is noted for reproducibility. |

## PDF-parsing notes (resolved)

| Source | Note |
|---|---|
| Direct Plastics HDPE, Treatstock PC, Alro Delrin PDFs | WebFetch could not parse the binary PDF streams ("corrupted/encoded"). Resolved by reading the locally-cached PDF copies with the Read tool, which parsed them cleanly. Values captured. |
| USDA FPL Wood Handbook GTR-190 | Full PDF (>10 MB at research.fs.usda.gov) exceeded WebFetch content limit; the 2.1 MB PreciseBits-hosted Ch.5 mirror was fetched, cached, and parsed with `pdftotext -layout`. Side-hardness Tables 5-3a/5-3b extracted successfully. |

## True data GAPS (no citeable value found)

| Material | What was sought | Reason it is a gap |
|---|---|---|
| MDF | Janka hardness | Janka (ASTM D143) is a solid-wood test; MDF is not Janka-rated. PreciseBits table explicitly states "MDF and particleboard are not included in this table." No Grade-A Janka exists. Repo's `SheetGoodKind::Mdf` effective Janka (1100) is synthetic, not citeable. |
| Particleboard | Janka hardness | Same as MDF — engineered panel, no ASTM D143 Janka rating. Repo's `Particleboard` effective Janka (750) is synthetic. |
| HDF | Janka hardness | Same — engineered panel; repo `Hdf` (1300) is synthetic. |
| Southern Yellow Pine (loblolly) | Grade-A single-species Janka | Only Longleaf (870 lbf) was fetched as a Grade-A SYP-group member. Repo value 690 is below any published SYP Janka; if it tracks loblolly specifically, a loblolly Janka source was not captured this round. Flagged for follow-up, not invented. |
