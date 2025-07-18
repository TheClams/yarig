/*------------------------------------------------------------------------------
// Interface: axi4_lite_if
//  Standard AXI4 Lite interface
//
//----------------------------------------------------------------------------*/

interface axi4_lite_if #(parameter int ADDR_W=32, DATA_W=32) (input bit aclk, aresetn);
 
/*-------------------------------------------------------------------------------
-- Signals declaration
-------------------------------------------------------------------------------*/

   // Read Address Channel
   logic [ADDR_W-1:0] araddr ; // Read address
   logic [       3:0] arcache; // Memory type. b0= bufferable, b1=cacheable, b2=read allocation, b3=write allocation
   logic [       2:0] arprot ; // Protection type. b0=secure, b1=privileged, b2=kind (instruction(1)/data(0)
   logic              arvalid; // Read address valid. Master generates this signal when Read Address and the control signals are valid.
   logic              arready; // Read address ready. Slave generates this signal when it can accept the read address and control signals.

   // Read Data Channel
   logic [DATA_W-1:0] rdata ; // Read Data
   logic [       1:0] rresp ; // Read response. This signal indicates the status of data transfer.
   logic              rvalid; // Read valid. Slave generates this signal when Read Data is valid
   logic              rready; // Read ready. Master generates this signal when it can accept the Read Data and response.

   // Write Address Channel
   logic [ADDR_W-1:0] awaddr ; // Write address, usually 32-bits wide.
   logic [       3:0] awcache; // Memory type. b0= bufferable, b1=cacheable, b2=read allocation, b3=write allocation
   logic [       2:0] awprot ; // Protection type. b0=secure, b1=privileged, b2=kind (instruction(1)/data(0)
   logic              awvalid; // Write address valid. Master generates this signal when Write Address and control signals are valid
   logic              awready; // Write address ready. Slave generates this signal when it can accept Write Address and control signals

   // Write Data Channel
   logic [  DATA_W-1:0] wdata; // Write data
   logic [DATA_W/8-1:0] wstrb; // Write strobes. 1b per byte

   // Write Response Channel
   logic [1:0] bresp ; // Write response. This signal indicates the status of the write transaction.
   logic       bvalid; // Write response valid. Slave generates this signal when the write response on the bus is valid.
   logic       bready; // Response ready. Master generates this signal when it can accept a write response

   typedef enum logic [1:0] {
      RESP_OKAY   = 2'b00, // Normal access succeed  (or exclusive access failed)
      RESP_EXOKAY = 2'b01, // Exclusive access succeed
      RESP_SLVERR = 2'b10, // Slave error
      RESP_DECERR = 2'b11, // Decoding error (No slave at the address)
   } e_resp;

/*------------------------------------------------------------------------------
--  Modports
------------------------------------------------------------------------------*/
   modport master (
      output araddr, arcache, arprot, arvalid, input arready,
      input  rdata, rresp, rvalid, output rready,
      output awaddr, awcache, awprot, awvalid, input awready,
      output wdata, wstrb,
      input  bresp, bvalid, output bready
   );

   modport slave (
      input  araddr, arcache, arprot, arvalid, output arready,
      output rdata, rresp, rvalid, input rready,
      input  awaddr, awcache, awprot, awvalid, output awready,
      input  wdata, wstrb,
      output bresp, bvalid, input bready
   );

   modport spy (
      input araddr, arcache, arprot, arvalid, arready,
      input rdata, rresp, rvalid, rready,
      input awaddr, awcache, awprot, awvalid, awready,
      input wdata, wstrb,
      input bresp, bvalid, bready
   );

/*-------------------------------------------------------------------------------
-- Clocking Blocks
-------------------------------------------------------------------------------*/
`ifndef SYNTHESIS
   default clocking cb @(posedge clk);
      input araddr, arcache, arprot, arvalid, arready,
      input rdata, rresp, rvalid, rready,
      input awaddr, awcache, awprot, awvalid, awready,
      input wdata, wstrb,
      input bresp, bvalid, bready
   endclocking
`endif

/*-------------------------------------------------------------------------------
--  Coverage & Assertions
-------------------------------------------------------------------------------*/

endinterface: axi4_lite_if
