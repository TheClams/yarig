--------------------------------------------------------------------------------
-- entity: bridge_vhd_apb_rif
--  Bridge between RIF Gen interface and APB
--  The option ASSUM_WR_OK allows to avoid some wait cycle for the write to be fully done
--
--------------------------------------------------------------------------------
library ieee;
use ieee.std_logic_1164.all;
use ieee.std_logic_misc.all;

entity bridge_vhd_apb_rif is
  generic (ADDR_W : natural := 16; DATA_W : natural := 32; ASSUME_WR_OK : boolean := false);
  port (
    clk : in std_logic; -- Clock
    -- Input signals
    rst_n : in std_logic;
    -- RIF  
    reg_addr         : out std_logic_vector(ADDR_W-1 downto 0); --Register Address
    reg_en           : out std_logic                    ; --Register Enable
    reg_rd_wrn       : out std_logic                    ; --Register write/not read
    reg_wr_data      : out std_logic_vector(DATA_W-1 downto 0); --Register write data
    reg_rd_data      : in  std_logic_vector(DATA_W-1 downto 0); --Register read data
    reg_done         : in  std_logic                    ; --Register ready
    reg_done_next    : in  std_logic                    ; --Register ready next
    reg_err_addr     : in  std_logic                    ; --Register address error
    reg_err_access   : in  std_logic                    ; --Register access error

    -- Register SW Interface
    paddr          : in  std_logic_vector(ADDR_W-1 downto 0); --APB Address
    psel           : in  std_logic                    ; --APB Select
    penable        : in  std_logic                    ; --APB Enable
    pwrite         : in  std_logic                    ; --APB write/not read
    pwdata         : in  std_logic_vector(DATA_W-1 downto 0); --APB write data
    pready         : out std_logic                    ; --APB ready
    prdata         : out std_logic_vector(DATA_W-1 downto 0); --APB read data
    pslverr        : out std_logic                      --APB slave error
  );
end bridge_vhd_apb_rif;

architecture rtl of bridge_vhd_apb_rif is

--------------------------------------------------------------------------------
--  Signals declaration
--------------------------------------------------------------------------------
  signal wr_en  : std_logic;

begin

  -- Generate a pulse of write enable
  proc_wr_en : process(clk, rst_n) begin
    if rst_n = '0' then
      wr_en <= '0';
    elsif rising_edge(clk) then
      wr_en <= psel and not penable and pwrite;        
    end if;
  end process;

  reg_addr    <= paddr;
  reg_en      <= (psel and not pwrite and not penable) or wr_en;
  reg_rd_wrn  <= not pwrite;
  reg_wr_data <= pwdata;
  prdata      <= reg_rd_data; 

  G_ASSUME_WR_OK : if (ASSUME_WR_OK = true) generate
    pslverr <= reg_done and reg_err_addr and not pwrite;
    pready  <= '0' when (penable = '1' and pwrite = '0' and reg_done = '0') else '1';
  end generate G_ASSUME_WR_OK;
  
  G_ASSUME_NOT_WR_OK : if (ASSUME_WR_OK = false) generate 
    pslverr <= reg_done and (reg_err_addr or reg_err_access);
    pready  <= '0' when (penable = '1' and reg_done = '0') else '1';
  end generate G_ASSUME_NOT_WR_OK;

end architecture;
