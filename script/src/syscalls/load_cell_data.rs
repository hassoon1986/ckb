use crate::{
    syscalls::{
        utils::store_data, Source, SourceEntry, INDEX_OUT_OF_BOUND,
        LOAD_CELL_DATA_AS_CODE_SYSCALL_NUMBER, LOAD_CELL_DATA_SYSCALL_NUMBER, SLICE_OUT_OF_BOUND,
        SUCCESS,
    },
    DataLoader,
};
use ckb_core::cell::CellMeta;
use ckb_vm::{
    memory::{Memory, FLAG_EXECUTABLE, FLAG_FREEZED},
    registers::{A0, A1, A2, A3, A4, A5, A7},
    Error as VMError, Register, SupportMachine, Syscalls,
};

pub struct LoadCellData<'a, DL> {
    data_loader: &'a DL,
    outputs: &'a [CellMeta],
    resolved_inputs: &'a [CellMeta],
    resolved_cell_deps: &'a [CellMeta],
    group_inputs: &'a [usize],
    group_outputs: &'a [usize],
}

impl<'a, DL: DataLoader + 'a> LoadCellData<'a, DL> {
    pub fn new(
        data_loader: &'a DL,
        outputs: &'a [CellMeta],
        resolved_inputs: &'a [CellMeta],
        resolved_cell_deps: &'a [CellMeta],
        group_inputs: &'a [usize],
        group_outputs: &'a [usize],
    ) -> LoadCellData<'a, DL> {
        LoadCellData {
            data_loader,
            outputs,
            resolved_inputs,
            resolved_cell_deps,
            group_inputs,
            group_outputs,
        }
    }

    fn fetch_cell(&self, source: Source, index: usize) -> Result<&'a CellMeta, u8> {
        match source {
            Source::Transaction(SourceEntry::Input) => {
                self.resolved_inputs.get(index).ok_or(INDEX_OUT_OF_BOUND)
            }
            Source::Transaction(SourceEntry::Output) => {
                self.outputs.get(index).ok_or(INDEX_OUT_OF_BOUND)
            }
            Source::Transaction(SourceEntry::CellDep) => {
                self.resolved_cell_deps.get(index).ok_or(INDEX_OUT_OF_BOUND)
            }
            Source::Transaction(SourceEntry::HeaderDep) => Err(INDEX_OUT_OF_BOUND),
            Source::Group(SourceEntry::Input) => self
                .group_inputs
                .get(index)
                .ok_or(INDEX_OUT_OF_BOUND)
                .and_then(|actual_index| {
                    self.resolved_inputs
                        .get(*actual_index)
                        .ok_or(INDEX_OUT_OF_BOUND)
                }),
            Source::Group(SourceEntry::Output) => self
                .group_outputs
                .get(index)
                .ok_or(INDEX_OUT_OF_BOUND)
                .and_then(|actual_index| self.outputs.get(*actual_index).ok_or(INDEX_OUT_OF_BOUND)),
            Source::Group(SourceEntry::CellDep) => Err(INDEX_OUT_OF_BOUND),
            Source::Group(SourceEntry::HeaderDep) => Err(INDEX_OUT_OF_BOUND),
        }
    }

    fn load_data_as_code<Mac: SupportMachine>(&self, machine: &mut Mac) -> Result<(), VMError> {
        let addr = machine.registers()[A0].to_u64();
        let memory_size = machine.registers()[A1].to_u64();
        let content_offset = machine.registers()[A2].to_u64();
        let content_size = machine.registers()[A3].to_u64();

        let index = machine.registers()[A4].to_u64();
        let source = Source::parse_from_u64(machine.registers()[A5].to_u64())?;

        let cell = self.fetch_cell(source, index as usize);
        if cell.is_err() {
            machine.set_register(A0, Mac::REG::from_u8(cell.unwrap_err()));
            return Ok(());
        }
        let cell = cell.unwrap();

        if content_offset >= cell.data_bytes
            || (content_offset + content_size) > cell.data_bytes
            || content_size > memory_size
        {
            machine.set_register(A0, Mac::REG::from_u8(SLICE_OUT_OF_BOUND));
            return Ok(());
        }
        let data = self
            .data_loader
            .load_cell_data(cell)
            .ok_or(VMError::Unexpected)?;
        machine.memory_mut().init_pages(
            addr,
            memory_size,
            FLAG_EXECUTABLE | FLAG_FREEZED,
            Some(data.slice(
                content_offset as usize,
                (content_offset + content_size) as usize,
            )),
            0,
        )?;

        machine.add_cycles(cell.data_bytes * 10)?;
        machine.set_register(A0, Mac::REG::from_u8(SUCCESS));
        Ok(())
    }

    fn load_data<Mac: SupportMachine>(&self, machine: &mut Mac) -> Result<(), VMError> {
        let index = machine.registers()[A3].to_u64();
        let source = Source::parse_from_u64(machine.registers()[A4].to_u64())?;

        let cell = self.fetch_cell(source, index as usize);
        if cell.is_err() {
            machine.set_register(A0, Mac::REG::from_u8(cell.unwrap_err()));
            return Ok(());
        }
        let cell = cell.unwrap();
        let data = self
            .data_loader
            .load_cell_data(cell)
            .ok_or(VMError::Unexpected)?;

        let wrote_size = store_data(machine, &data)?;
        machine.add_cycles(wrote_size * 10)?;
        machine.set_register(A0, Mac::REG::from_u8(SUCCESS));
        Ok(())
    }
}

impl<'a, Mac: SupportMachine, DL: DataLoader> Syscalls<Mac> for LoadCellData<'a, DL> {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), VMError> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, VMError> {
        let code = machine.registers()[A7].to_u64();
        if code == LOAD_CELL_DATA_AS_CODE_SYSCALL_NUMBER {
            self.load_data_as_code(machine)?;
            return Ok(true);
        } else if code == LOAD_CELL_DATA_SYSCALL_NUMBER {
            self.load_data(machine)?;
            return Ok(true);
        }
        Ok(false)
    }
}
