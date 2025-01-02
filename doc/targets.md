# Targets

This page describes information about the different generator outputs (except hardwrae which has its own dedicated page).

---
## C
For each register file definition the generator outputs a header file containing:

 - One structure per register (union between "uint32 reg32" and a struct of bitfields called fields)
 - Macro definition for each field giving their position and mask
 - One structure per page grouping all register together, and inserting reserved register when needed

The naming convention is:

 - Register struct: RifName + RegisterName + "Reg_u" (using PascalCase)
 - Field name inside register struct uses camelCase
 - Macro: RIFNAME_REGNAME_FIELDNAME + "\_POS"|"\_MASK"|"\_SMASK"
 - RIF struct: RifName + "Regs" (using PascalCase).
 - Register name inside RIF struct uses camelCase

If a rifmux is provided, an additional file is generated including all others header files
and defining macro for base address and pointer to those addresses.

The naming convention is:

 - Base address : RIFINSTNAME_BASE_ADDR
 - Pointer : P_RIFINSTNAME

When there is more than one page in a RIF, the page name is appended to the RIF name.

---
## LaTeX

The latex target has labels for each register and field register.
To easily reference them, here is an example of command that add hyperlink and proper formatting.

```
\makeatletter
\DeclareRobustCommand*{\escapeus}[1]{%
  \begingroup\@activeus\scantokens{#1\endinput}\endgroup}
\begingroup\lccode`\~=`\_\relax
   \lowercase{\endgroup\def\@activeus{\catcode`\_=\active \let~\_}}
\makeatother

% Define command to display registers properly and with a link to the RIF table
\newcounter{rif}
\newcommand{\reg}[1]{\hyperref[rif:rifname.#1]{\texttt{\escapeus{#1}}}}
```