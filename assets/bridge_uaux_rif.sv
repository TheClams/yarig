/*------------------------------------------------------------------------------
// module: bridge_uaux_rif
//  Bridge between RIF Gen interface and ARC User AUX register interface 
//
//----------------------------------------------------------------------------*/
`default_nettype none

module bridge_uaux_rif #(parameter int ADDR_W=16, DATA_W=32) (
  // RIF Interface //
  rif_if.ctrl                 if_rif ,
  
  // User AUX Interface //
  input  wire                uaux_en_r  ,            //  Aux enable
  input  wire [ADDR_W-1:0]   uaux_addr_r  ,          //  Aux Address
  input  wire                uaux_read,              //  UAUX LR
  input  wire                uaux_write,             //  UAUX SR

  output wire                uaux_busy  ,            //  Structural hazard
  output wire [DATA_W-1:0]   uaux_rdata,             //  LR read data
  output wire                uaux_illegal,           //  SR/LR illegal
  output wire                uaux_k_rd,              //  need Kernel Rd
  output wire                uaux_k_wr,              //  need Kernel Wr
  output wire                uaux_unimpl,            //  Invalid Reg
  output wire                uaux_serial_sr,         //  SR group flush
  output wire                uaux_strict_sr,         //  SR single flus  h

  input  wire                uaux_cmt_phase  ,       //  UAUX Commit status
  input  wire                uaux_cmt_valid  ,       //  UAUX Commit Valid
  input  wire [DATA_W-1:0]   uaux_wdata_r            //  (CA) Aux Write Data
);

/*------------------------------------------------------------------------------
-- Signals declaration
------------------------------------------------------------------------------*/
  logic uaux_r_mask; // Request mask
  logic uaux_read_d; 
  logic uaux_write_d; 
  logic rif_en;

/*------------------------------------------------------------------------------
-- Interface Conversion 
------------------------------------------------------------------------------*/
  always_ff @(posedge if_rif.clk or negedge if_rif.rst_n) begin : uaux_en_reg
    if (~if_rif.rst_n) begin 
      uaux_read_d  <= 1'b0;  
      uaux_write_d <= 1'b0;  
    end
    else begin
      uaux_read_d  <= uaux_read  && uaux_busy;
      uaux_write_d <= uaux_write && uaux_busy;
    end
  end : uaux_en_reg

  // Read/Write pulse masked by enable
  assign rif_en = uaux_en_r && ((uaux_read && ~uaux_read_d) || (uaux_write && ~uaux_write_d));

  assign if_rif.addr    =  uaux_addr_r;
  assign if_rif.en      =  rif_en;
  assign if_rif.rd_wrn  = ~uaux_write; 
  assign if_rif.wr_data =  uaux_wdata_r;
  
  assign uaux_rdata     =  if_rif.rd_data;
  assign uaux_busy      = ~if_rif.done;

  // When uaux_busy=0 and uaux_unimlp=1 all response group signals need to be 0 
  assign uaux_r_mask = (uaux_busy | ~uaux_unimpl);

  assign uaux_unimpl    = if_rif.err_addr;
  assign uaux_illegal   = if_rif.err_access & uaux_r_mask;
  assign uaux_k_rd      = 1'b0;
  assign uaux_k_wr      = 1'b0;
  assign uaux_serial_sr = 1'b0;
  assign uaux_strict_sr = 1'b0;

endmodule : bridge_uaux_rif
`default_nettype wire

