/*------------------------------------------------------------------------------
// module: bridge_uaux_rif
//  Bridge between RIF Gen interface and ARC User AUX register interface 
//
//----------------------------------------------------------------------------*/

module bridge_uaux_rif #(parameter int ADDR_W=16, DATA_W=32) (
  rif_if.ctrl                   if_rif        , // SW register interface
  // User AUX Interface //
  input  var logic              uaux_en_r       , // AUX enable
  input  var logic [ADDR_W-1:0] uaux_addr_r     , // AUX address
  input  var logic              uaux_read     , // AUX read
  input  var logic              uaux_write    , // AUX write
  output var logic              uaux_busy     , // AUX busy
  output var logic [DATA_W-1:0] uaux_rdata    , // AUX read data
  output var logic              uaux_illegal  , // SR/LR illegal
  output var logic              uaux_k_rd     , // AUX read privilege violation
  output var logic              uaux_k_wr     , // AUX write privilege violation
  output var logic              uaux_unimpl   , // AUX unimplemented address
  output var logic              uaux_serial_sr, // AUX SR group flush
  output var logic              uaux_strict_sr, // AUX SR single flush
  input  var logic              uaux_cmt_phase, // AUX commit status
  input  var logic              uaux_cmt_valid, // AUX commit valid
  input  var logic [DATA_W-1:0] uaux_wdata_r      // AUX write data
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
