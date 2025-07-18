/*------------------------------------------------------------------------------
// Interface: apb_if
//  Standard APB interface
//
//----------------------------------------------------------------------------*/

interface apb_if #(parameter int ADDR_W=32, DATA_W=32) (input bit pclk, presetn);
 
/*-------------------------------------------------------------------------------
-- Signals declaration
-------------------------------------------------------------------------------*/
   logic [  ADDR_W-1:0] paddr;   // APB Address
   logic                psel;    // APB Select
   logic                penable; // APB Enable
   logic                pready;  // APB ready
   logic                pwrite;  // APB write/not read
   logic [  DATA_W-1:0] prdata;  // APB read data
   logic [  DATA_W-1:0] pwdata;  // APB write data
   logic [DATA_W/8-1:0] pstrb;   // APB byte enable write strobe (optionnal)
   logic                pslverr; // Slave error (mandatory)

/*------------------------------------------------------------------------------
--  Modports
------------------------------------------------------------------------------*/
   modport master (
      input  pclk, presetn, prdata, pready, pslverr,
      output penable, psel, pwrite, pwdata, pstrb, paddr
   );

   modport slave (
      input  pclk, presetn, penable, psel, pwrite, pwdata, pstrb, paddr,
      output prdata, pready, pslverr
   );

   modport spy (
      input pclk, presetn, penable, psel, pwrite, pwdata, pstrb, paddr, prdata, pready, pslverr
   );

/*-------------------------------------------------------------------------------
-- Clocking Blocks
-------------------------------------------------------------------------------*/
`ifndef SYNTHESIS
   default clocking cb @(posedge clk);
      input penable, psel, pwrite, pwdata, pstrb, paddr, prdata, pready, pslverr;
   endclocking
`endif

/*-------------------------------------------------------------------------------
--  Coverage & Assertions
-------------------------------------------------------------------------------*/

endinterface: apb_if
