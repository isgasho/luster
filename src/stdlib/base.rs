use std::io::{self, Write};

use gc_arena::MutationContext;

use crate::{
    sequence_fn_with, Callback, CallbackResult, IntoSequence, LuaContext, RuntimeError,
    SequenceExt, String, Table, TypeError, Value,
};

pub fn load_base<'gc>(mc: MutationContext<'gc, '_>, _: LuaContext<'gc>, env: Table<'gc>) {
    env.set(
        mc,
        String::new_static(b"print"),
        Callback::new_immediate(mc, |_, args| {
            let mut stdout = io::stdout();
            for i in 0..args.len() {
                args[i].display(&mut stdout)?;
                if i != args.len() - 1 {
                    stdout.write_all(&b"\t"[..])?;
                }
            }
            stdout.write_all(&b"\n"[..])?;
            stdout.flush()?;
            Ok(CallbackResult::Return(vec![]))
        }),
    )
    .unwrap();

    env.set(
        mc,
        String::new_static(b"error"),
        Callback::new_immediate(mc, |_, args| {
            let err = args.get(0).cloned().unwrap_or(Value::Nil);
            Err(RuntimeError(err).into())
        }),
    )
    .unwrap();

    env.set(
        mc,
        String::new_static(b"pcall"),
        Callback::new(mc, |thread, mut args| {
            let function = match args.get(0).cloned().unwrap_or(Value::Nil) {
                Value::Function(function) => function,
                value => {
                    return Box::new(
                        Err(TypeError {
                            expected: "function",
                            found: value.type_name(),
                        }
                        .into())
                        .into_sequence(),
                    );
                }
            };

            // TODO: should be able to yield through pcall, requires tail-calling functions

            args.remove(0);
            Box::new(sequence_fn_with(
                (thread, function, args),
                |mc, _, (thread, function, args)| {
                    thread.call(mc, function, &args).then(|mc, lc, res| {
                        Ok(CallbackResult::Return(match res {
                            Ok(mut res) => {
                                res.insert(0, Value::Boolean(true));
                                res
                            }
                            Err(err) => {
                                vec![Value::Boolean(false), err.to_value(mc, lc.interned_strings)]
                            }
                        }))
                    })
                },
            ))
        }),
    )
    .unwrap();
}