------------------------------------------------------------------------------
-- module: bridge_vhd_uaux_rif
-- Bridge between RIF Gen interface and ARC User AUX register interface 
--
------------------------------------------------------------------------------

library ieee;
use ieee.std_logic_1164.all;
use ieee.std_logic_misc.all;

entity bridge_vhd_uaux_rif is
  generic (ADDR_W : natural := 16; DATA_W : natural := 32; ASSUME_WR_OK : boolean := false);
  port (
    clk            : in  std_logic;     -- Clock
    -- Input signals
    rst_n          : in  std_logic;
    -- RIF  
    reg_addr       : out std_logic_vector(ADDR_W-1 downto 0);  -- Register Address
    reg_en         : out std_logic;     -- Register Enable
    reg_rd_wrn     : out std_logic;     -- Register write/not read
    reg_wr_data    : out std_logic_vector(DATA_W-1 downto 0);  -- Register write data
    reg_rd_data    : in  std_logic_vector(DATA_W-1 downto 0);  -- Register read data
    reg_done       : in  std_logic;     -- Register ready
    reg_done_next  : in  std_logic;     -- Register ready next
    reg_err_addr   : in  std_logic;     -- Register address error
    reg_err_access : in  std_logic;     -- Register access error


    -- User AUX Interface 
    uaux_en_r   : in std_logic;                            --  Aux enable
    uaux_addr_r : in std_logic_vector(ADDR_W-1 downto 0);  --  Aux Address
    uaux_read   : in std_logic;                            --  UAUX LR
    uaux_write  : in std_logic;                            --  UAUX SR

    uaux_busy      : out std_logic;     --  Structural hazard
    uaux_rdata     : out std_logic_vector(DATA_W-1 downto 0);  --  LR read data
    uaux_illegal   : out std_logic;     --  SR/LR illegal
    uaux_k_rd      : out std_logic;     --  need Kernel Rd
    uaux_k_wr      : out std_logic;     --  need Kernel Wr
    uaux_unimpl    : out std_logic;     --  Invalid Reg
    uaux_serial_sr : out std_logic;     --  SR group flush
    uaux_strict_sr : out std_logic;     --  SR single flus  h

    uaux_cmt_phase : in std_logic;      --  UAUX Commit status
    uaux_cmt_valid : in std_logic;      --  UAUX Commit Valid
    uaux_wdata_r   : in std_logic_vector(DATA_W-1 downto 0)  --  (CA) Aux Write Data
    );
end bridge_vhd_uaux_rif;

architecture rtl of bridge_vhd_uaux_rif is

--------------------------------------------------------------------------------
-- Signals declaration
--------------------------------------------------------------------------------
  signal uaux_r_mask   : std_logic;       -- Request mask
  signal uaux_busy_i   : std_logic;
  signal uaux_unimpl_i : std_logic;
  signal uaux_read_d   : std_logic;
  signal uaux_write_d  : std_logic;
  signal rif_en        : std_logic;
begin

--------------------------------------------------------------------------------
-- Interface Conversion 
--------------------------------------------------------------------------------
  uaux_en_reg : process(clk, rst_n) begin
    if rst_n = '0' then
      uaux_read_d  <= '0';
      uaux_write_d <= '0';
    elsif rising_edge(clk) then
      uaux_read_d  <= uaux_read  and uaux_busy_i;
      uaux_write_d <= uaux_write and uaux_busy_i;
    end if;
  end process;

  -- Read/Write pulse masked by enable
  rif_en      <= uaux_en_r and ((uaux_read and not uaux_read_d) or (uaux_write and not uaux_write_d));

  reg_addr    <= uaux_addr_r;
  reg_en      <= uaux_en_r;
  reg_rd_wrn  <= not uaux_write;
  reg_wr_data <= uaux_wdata_r;

  uaux_rdata   <= reg_rd_data;
  uaux_busy_i  <= not reg_done; 
  uaux_busy    <= uaux_busy_i;

  -- When uaux_busy=0 and uaux_unimlp=1 all response group signals need to be 0 
  uaux_r_mask <= (uaux_busy_i or not uaux_unimpl_i);

  uaux_unimpl_i  <= reg_err_addr;
  uaux_unimpl    <= uaux_unimpl_i;

  uaux_illegal   <= reg_err_access and uaux_r_mask;
  uaux_k_rd      <= '0';
  uaux_k_wr      <= '0';
  uaux_serial_sr <= '0';
  uaux_strict_sr <= '0';


end architecture;
