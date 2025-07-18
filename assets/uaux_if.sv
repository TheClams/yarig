/*------------------------------------------------------------------------------
// Interface: uaux_if
//  Standard ARC User AUX interface
//
//----------------------------------------------------------------------------*/

interface uaux_if #(parameter int ADDR_W=32, DATA_W=32) (input bit pclk, presetn);
 
/*-------------------------------------------------------------------------------
-- Signals declaration
-------------------------------------------------------------------------------*/

   logic              en_r;      //  Aux enable
   logic [ADDR_W-1:0] addr_r;    //  Aux Address
   logic              read;      //  UAUX LR
   logic              write;     //  UAUX SR
   logic              busy;      //  Structural hazard
   logic [DATA_W-1:0] rdata;     //  LR read data
   logic              illegal;   //  SR/LR illegal
   logic              k_rd;      //  need Kernel Rd
   logic              k_wr;      //  need Kernel Wr
   logic              unimpl;    //  Invalid Reg
   logic              serial_sr; //  SR group flush
   logic              strict_sr; //  SR single flush
   logic              cmt_phase; //  UAUX Commit status
   logic              cmt_valid; //  UAUX Commit Valid
   logic [DATA_W-1:0] wdata_r;   //  (CA) Aux Write Data

/*------------------------------------------------------------------------------
--  Modports
------------------------------------------------------------------------------*/
   modport master (
      output en_r, addr_r, read, write, cmt_phase, cmt_valid, wdata_r,
      input  busy, rdata, illegal, k_rd, k_wr, unimpl, serial_sr, strict_sr
   );

   modport slave (
      input  en_r, addr_r, read, write, cmt_phase, cmt_valid, wdata_r,
      output busy, rdata, illegal, k_rd, k_wr, unimpl, serial_sr, strict_sr
   );

   modport spy (
      input en_r, addr_r, read, write, cmt_phase, cmt_valid, wdata_r,
      input busy, rdata, illegal, k_rd, k_wr, unimpl, serial_sr, strict_sr
   );

/*-------------------------------------------------------------------------------
-- Clocking Blocks
-------------------------------------------------------------------------------*/
`ifndef SYNTHESIS
   default clocking cb @(posedge clk);
      input en_r, addr_r, read, write, cmt_phase, cmt_valid, wdata_r,
      input busy, rdata, illegal, k_rd, k_wr, unimpl, serial_sr, strict_sr
   endclocking
`endif

/*-------------------------------------------------------------------------------
--  Coverage & Assertions
-------------------------------------------------------------------------------*/

endinterface: uaux_if
