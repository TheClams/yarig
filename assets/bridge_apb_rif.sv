/*------------------------------------------------------------------------------
// module: bridge_apb_rif
//  Bridge between RIF Gen interface and APB
//
//----------------------------------------------------------------------------*/

module bridge_apb_rif #(parameter int ADDR_W=16,parameter int DATA_W=32) (
   rif_if.ctrl              if_rif , // SW register interface
   // APB Interface
   input  var logic [ADDR_W-1:0] paddr  , // APB Address
   input  var logic              psel   , // APB Select
   input  var logic              penable, // APB Enable
   input  var logic              pwrite , // APB Write
   input  var logic [DATA_W-1:0] pwdata , // APB Write Data
   output var logic [DATA_W-1:0] prdata , // APB Read Data
   output var logic              pready , // APB Ready
   output var logic              pslverr  // APB Slave Error
);

/*------------------------------------------------------------------------------
--
------------------------------------------------------------------------------*/

   assign if_rif.addr = paddr;
   assign if_rif.en   = psel & ~penable; // Setup phase
   assign if_rif.rd_wrn = ~pwrite;
   assign if_rif.wr_data = pwdata;

   assign prdata = if_rif.rd_data;

   assign pslverr = if_rif.done & (if_rif.err_addr | if_rif.err_access);
   assign pready  = penable & psel & ~if_rif.done ? 1'b0 : 1'b1;

endmodule
