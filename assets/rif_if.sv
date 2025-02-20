/*------------------------------------------------------------------------------
// Interface: rif_if
//  Internal Interface for the RIF generator
//
//----------------------------------------------------------------------------*/

interface rif_if #(parameter W_ADDR=16, W_DATA=32) (input bit clk, rst_n);

/*-------------------------------------------------------------------------------
-- Signals declaration
-------------------------------------------------------------------------------*/
   logic              en        ; // High to access RIF
   logic              rd_wrn    ; // High for read, low for write
   logic [W_ADDR-1:0] addr      ; // Address
   logic [W_DATA-1:0] wr_data   ; // Write data
   logic [W_DATA-1:0] rd_data   ; // Read data
   logic              done      ; // Pulse high when access is complete
   logic              err_addr  ; // High if access failed due to invalid address
   logic              err_access; // High if access failed due to rd/wr access status
   // Combinatorial version done/err signals
   logic              done_next      ; // Pulse high when access is about to complete
   logic              err_addr_next  ; // High if access is failing due to invalid address (valid on done_next)
   logic              err_access_next; // High if access is failing due to rd/wr access status (valid on done_next)

/*------------------------------------------------------------------------------
--  Modports
------------------------------------------------------------------------------*/
   modport rif  (input  rst_n, clk, en, rd_wrn, addr, wr_data, output rd_data, done, err_addr, err_access, done_next, err_addr_next, err_access_next);
   modport ctrl (output en, rd_wrn, addr, wr_data, input  rst_n, clk, rd_data, done, err_addr, err_access, done_next, err_addr_next, err_access_next);

/*-------------------------------------------------------------------------------
-- Clocking Blocks
-------------------------------------------------------------------------------*/
`ifndef SYNTHESIS
   default clocking cb @(posedge clk);
      input rst_n, en, rd_wrn, addr, wr_data, rd_data, done, err_addr, err_access, done_next, err_addr_next, err_access_next;
   endclocking
`endif

/*-------------------------------------------------------------------------------
--  Coverage & Assertions
-------------------------------------------------------------------------------*/

endinterface : rif_if
