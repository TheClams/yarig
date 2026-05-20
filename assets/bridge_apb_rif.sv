/*------------------------------------------------------------------------------
// module: bridge_apb_rif
//  Bridge between RIF Gen interface and APB
//  The option ASSUME_WR_OK allows to avoid some wait cycle for the write to be fully done
//
//----------------------------------------------------------------------------*/

module bridge_apb_rif #(parameter int ADDR_W=16,parameter int DATA_W=32) (
   input  wire              clk    ,
   input  wire              rst_n  ,
   rif_if.ctrl              if_rif ,
   // APB Interface
   input  wire [ADDR_W-1:0] paddr  ,
   input  wire              psel   ,
   input  wire              penable,
   input  wire              pwrite ,
   input  wire [DATA_W-1:0] pwdata ,
   output wire [DATA_W-1:0] prdata ,
   output wire              pready ,
   output wire              pslverr
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
   assign pready  = penable & ~if_rif.done ? 1'b0 : 1'b1;

endmodule
