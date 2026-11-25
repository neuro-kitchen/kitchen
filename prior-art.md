# Prior art

Pipelines, provenance and scientific data formats have been standardized many times. What each got
right, what Strata takes, and why none of them is the contract on its own.

## Workflow and pipeline systems

| | What it standardizes | What we take | Why it is not enough |
|---|---|---|---|
| **CWL** (Common Workflow Language) | Command-line tools and workflows, typed inputs and outputs, as files | Step descriptions with typed ports; a pipeline as data; portability between engines | Whole files and command lines only: no pieces and margins, no in-process steps on a device, no previews |
| **Nextflow, Snakemake** | Pipelines as code (a DSL), run on laptops to clusters | Caching by inputs (our run keys); resuming; the same pipeline local and on a cluster | Pipelines are programs, not data an interface can edit; each engine its own |
| **Galaxy** | Tools with XML descriptions, a web interface, histories of results | Tool descriptions that generate their own forms; histories as provenance a scientist can read | One big server application; tools are command lines |
| **GA4GH TRS / WES / DRS** | Small APIs: tool registry, workflow execution, data access | Small, separate services; IDs for data | Genomics, whole-file workflows |
| **OpenLineage** | Lineage events from data pipelines (jobs, runs, datasets) | Run records as events: a run, its inputs, its outputs | Data engineering, not analysis with parameters and people deciding |

## Provenance and packaging

| | What we take |
|---|---|
| **W3C PROV** | The model under run records: entities (results), activities (runs), agents (`by`, `via`). Lineage exports to PROV |
| **RO-Crate** | Packaging results with their provenance for a paper or an archive: lineage exports to an RO-Crate |
| **Git** | Content addressing: results and steps named by their hash; an identical input never computed twice |

## Neuroscience data

| | Role here |
|---|---|
| **NWB** (Neurodata Without Borders) | A format of `signal`, `events` and their metadata. Results can be stored as NWB; the contract does not replace it |
| **OME-Zarr / OME-TIFF** | Formats of `image`, `labels`; lucem's native ones |
| **BIDS** | How datasets are laid out; a source an app's domain can read |
| **DANDI** | The archive: lineage exported with a dataset makes it reusable |
| **SpikeInterface** | A Python layer over spike sorters with a common API: the same idea inside one domain; dsp-kitchen's steps can wrap its sorters |

## Protocols between programs

| | What we take |
|---|---|
| **LSP, MCP** | A capability handshake; one server, any client. MCP is the runtime API's transport for agents |
| **DICOM** | A format and a network protocol together, implemented by every vendor: the scale a contract can reach when it is small and required |

## The gap

None of them gives, together: steps described the same way whether built in, scripted, external or
composite; pipelines as data run piece by piece on a device; results with lineage across apps;
confirmation of generated code; and one API that an app's own screens, a generated interface and an
agent all use.
