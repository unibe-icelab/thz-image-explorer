---
title: 'THz Image Explorer - A Cross-Platform Open-Source THz Image Analysis Tool'
tags:
  - THz
  - Rust
  - Imaging
  - Data Analysis
authors:
  - name: Linus Leo Stöckli
    orcid: 0000-0002-7916-2592
    corresponding: true 
    affiliation: "1"
  - name: Nicolas Thomas
    orcid: 0000-0002-0146-0071
    affiliation: "1"
affiliations:
  - name: University of Bern, Bern, Switzerland
    index: 1
    ror: 02k7v4d05
date: 23 January 2025
bibliography: paper.bib

---

# Introduction

THz TDS, Images

# Statement of need

Interactive analysis, performance, open-source, cross-platform

# Structure

diagram of thread structure 

# Installation

Bundles for all platforms, dependencies, compilation instructions

# Usage

GUI layout, select pixels, different views

## IO

File loading, meta-data, cite dotTHz

## Filters

### FFT Window
discuss different windows

### Frequency Band Pass Filter
filter in frequency domain

### Time Domain Slice
slice through time axis - depth information of image

### Deconvolution

cite Arnaud's paper

### Extend filters

document changes required to implement custom filters



Single dollars ($) are required for inline mathematics e.g. $f(x) = e^{\pi/x}$

Double dollars make self-standing equations:

$$\Theta(x) = \left\{\begin{array}{l}
0\textrm{ if } x < 0\cr
1\textrm{ else}
\end{array}\right.$$

You can also use plain \LaTeX for equations
\begin{equation}\label{eq:fourier}
\hat f(\omega) = \int_{-\infty}^{\infty} f(x) e^{i\omega x} dx
\end{equation}
and refer to \autoref{eq:fourier} from text.

# Citations

Citations to entries in paper.bib should be in
[rMarkdown](http://rmarkdown.rstudio.com/authoring_bibliographies_and_citations.html)
format.

If you want to cite a software repository URL (e.g. something on GitHub without a preferred
citation) then you can do it with the example BibTeX entry below for @fidgit.

For a quick reference, the following citation commands can be used:

- `@author:2001`  ->  "Author et al. (2001)"
- `[@author:2001]` -> "(Author et al., 2001)"
- `[@author1:2001; @author2:2001]` -> "(Author1 et al., 2001; Author2 et al., 2002)"

# Figures

Figures can be included like this:
![Caption for example figure.\label{fig:example}](figure.png)
and referenced from text using \autoref{fig:example}.

Figure sizes can be customized by adding an optional second parameter:
![Caption for example figure.](figure.png){ width=20% }


# Outlook

talk about possible community contribution

# Acknowledgements

This work was supported through a MARVIS (Multidisciplinary Advanced 
Research Ventures in Space) programme of the Swiss Department for Business, Education,
and Research (SBFI) called SUBICE. SUBICE is a project of the University of Bern (UniBe), 
the University of Applied Sciences and Arts, Western Switzerland (HES-SO), and Thales-
Alenia Space Switzerland (TASCH). The project has been partially funded by the European 
Space Agency (ESA) under the ESA Initial Support for Innovation (EISI) program. 
We acknowledge the support of the Open Space Innovation Platform (OSIP) and in
particular Nicolas Thiry and Leopold Summerer. The contribution of RO and AP has been 
carried out within the framework of the NCCR PlanetS supported by the Swiss National 
Science Foundation under grants 51NF40-182901 and 51NF40-205606.

# References