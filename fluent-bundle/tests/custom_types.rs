use fluent_bundle::bundle::FormatterPass;
use fluent_bundle::memoizer::MemoizerKind;
use fluent_bundle::types::FluentType;
use fluent_bundle::FluentArgs;
use fluent_bundle::FluentBundle;
use fluent_bundle::FluentResource;
use fluent_bundle::FluentValue;
use unic_langid::langid;

#[test]
fn fluent_custom_type() {
    #[derive(Debug, PartialEq)]
    struct DateTime {
        epoch: usize,
    }

    impl DateTime {
        pub fn new(epoch: usize) -> Self {
            Self { epoch }
        }
    }

    impl FluentType for DateTime {
        fn duplicate(&self) -> Box<dyn FluentType + Send> {
            Box::new(DateTime { epoch: self.epoch })
        }
        fn as_string(&self, _: &intl_memoizer::IntlLangMemoizer) -> std::borrow::Cow<'static, str> {
            format!("{}", self.epoch).into()
        }
        fn as_string_threadsafe(
            &self,
            _: &intl_memoizer::concurrent::IntlLangMemoizer,
        ) -> std::borrow::Cow<'static, str> {
            format!("{}", self.epoch).into()
        }
    }

    let dt = FluentValue::Custom(Box::new(DateTime::new(10)));
    let dt2 = FluentValue::Custom(Box::new(DateTime::new(10)));
    let dt3 = FluentValue::Custom(Box::new(DateTime::new(15)));

    let sv = FluentValue::from("foo");

    assert!(dt == dt2);
    assert!(dt != dt3);
    assert!(dt != sv);
}

#[test]
fn fluent_date_time_builtin() {
    #[derive(Clone, Debug, Default, PartialEq)]
    enum DateTimeStyleValue {
        Full,
        Long,
        Medium,
        Short,
        #[default]
        None,
    }

    impl From<&FluentValue<'_>> for DateTimeStyleValue {
        fn from(input: &FluentValue) -> Self {
            if let FluentValue::String(s) = input {
                match s.as_ref() {
                    "full" => Self::Full,
                    "long" => Self::Long,
                    "medium" => Self::Medium,
                    "short" => Self::Short,
                    _ => Self::None,
                }
            } else {
                Self::None
            }
        }
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    struct DateTimeOptions {
        pub date_style: DateTimeStyleValue,
        pub time_style: DateTimeStyleValue,
    }

    impl DateTimeOptions {
        pub fn merge(&mut self, input: &FluentArgs) {
            for (key, value) in input.iter() {
                match key {
                    "dateStyle" => self.date_style = value.into(),
                    "timeStyle" => self.time_style = value.into(),
                    _ => {}
                }
            }
        }
    }

    impl From<&FluentArgs<'_>> for DateTimeOptions {
        fn from(input: &FluentArgs) -> Self {
            let mut opts = Self::default();
            opts.merge(input);
            opts
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct DateTime {
        epoch: usize,
        options: DateTimeOptions,
    }

    impl DateTime {
        pub fn new(epoch: usize, options: DateTimeOptions) -> Self {
            Self { epoch, options }
        }
    }

    impl FluentType for DateTime {
        fn duplicate(&self) -> Box<dyn FluentType + Send> {
            Box::new(DateTime::new(self.epoch, DateTimeOptions::default()))
        }
        fn as_string(&self, _: &intl_memoizer::IntlLangMemoizer) -> std::borrow::Cow<'static, str> {
            format!("2020-01-20 {}:00", self.epoch).into()
        }
        fn as_string_threadsafe(
            &self,
            _intls: &intl_memoizer::concurrent::IntlLangMemoizer,
        ) -> std::borrow::Cow<'static, str> {
            format!("2020-01-20 {}:00", self.epoch).into()
        }
    }

    let lang = langid!("en");
    let mut bundle = FluentBundle::new(vec![lang]);

    let res = FluentResource::try_new(
        r#"
key-explicit = Hello { DATETIME(12, dateStyle: "full") } World
key-ref = Hello { DATETIME($date, dateStyle: "full") } World
    "#
        .into(),
    )
    .unwrap();
    bundle.add_resource(res).unwrap();
    bundle.set_use_isolating(false);

    bundle
        .add_function("DATETIME", |positional, named| match positional.first() {
            Some(FluentValue::Custom(custom)) => {
                if let Some(that) = custom.as_ref().as_any().downcast_ref::<DateTime>() {
                    let mut dt = that.clone();
                    dt.options.merge(named);
                    FluentValue::Custom(Box::new(dt))
                } else {
                    FluentValue::Error
                }
            }
            Some(FluentValue::Number(num)) => {
                let num = num.value as usize;
                FluentValue::Custom(Box::new(DateTime::new(num, named.into())))
            }
            _ => FluentValue::Error,
        })
        .unwrap();

    let mut errors = vec![];
    let mut args = FluentArgs::new();
    args.set(
        "date",
        FluentValue::Custom(Box::new(DateTime::new(10, DateTimeOptions::default()))),
    );

    let msg = bundle.get_message("key-explicit").unwrap();
    let val = bundle.format_pattern(msg.value().unwrap(), Some(&args), &mut errors);
    assert_eq!(val, "Hello 2020-01-20 12:00 World");

    let msg = bundle.get_message("key-ref").unwrap();
    let val = bundle.format_pattern(msg.value().unwrap(), Some(&args), &mut errors);
    assert_eq!(val, "Hello 2020-01-20 10:00 World");
}

#[test]
fn fluent_custom_number_format() {
    fn custom_formatter<M: MemoizerKind>(
        num: &FluentValue,
        _intls: &M,
        _pass: FormatterPass,
    ) -> Option<String> {
        match num {
            FluentValue::Number(_) => Some("CUSTOM".into()),
            _ => None,
        }
    }

    let res = FluentResource::try_new(
        r#"
key-num-implicit = Hello { 5.000 } World
key-num-explicit = Hello { NUMBER(5, minimumFractionDigits: 2) } World
    "#
        .into(),
    )
    .unwrap();
    let mut bundle = FluentBundle::default();
    bundle.add_resource(res).unwrap();
    bundle.set_use_isolating(false);

    bundle
        .add_function("NUMBER", |positional, named| match positional.first() {
            Some(FluentValue::Number(n)) => {
                let mut num = n.clone();
                num.options.merge(named);
                FluentValue::Number(num)
            }
            _ => FluentValue::Error,
        })
        .unwrap();

    let mut errors = vec![];

    let msg = bundle.get_message("key-num-explicit").unwrap();
    let val = bundle.format_pattern(msg.value().unwrap(), None, &mut errors);
    assert_eq!(val, "Hello 5.00 World");

    let msg = bundle.get_message("key-num-implicit").unwrap();
    let val = bundle.format_pattern(msg.value().unwrap(), None, &mut errors);
    assert_eq!(val, "Hello 5.000 World");

    bundle.set_formatter(Some(custom_formatter));

    let msg = bundle.get_message("key-num-implicit").unwrap();
    let val = bundle.format_pattern(msg.value().unwrap(), None, &mut errors);
    assert_eq!(val, "Hello CUSTOM World");
}
