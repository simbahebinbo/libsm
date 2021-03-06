

use super::field::*;

use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::*;

use rand::os::OsRng;
use rand::Rng;

//定义结构体
pub struct EccCtx {
    fctx: FieldCtx,  //modp mod(-p)
    a: FieldElem, //域元素是8个u32
    b: FieldElem,
    pub n: BigUint,  //椭圆曲线生成元的阶 n
    inv2: FieldElem,  //域元素是8个u32
}

//雅克比坐标点
#[derive(Clone)]
pub struct Point {
    pub x: FieldElem,
    pub y: FieldElem,
    pub z: FieldElem,
}

//
fn pre_vec_gen(n: u32) -> [u32; 8] {
    let mut pre_vec: [u32; 8] = [0; 8];
    let mut i = 0;
    while i < 8 {  //i=0,1,2,3,4,5,6; 
        //pre-vec[7],pre-vec[7],pre-vec[6],pre-vec[5],pre-vec[4],pre-vec[3],pre-vec[2],pre-vec[1],pre-vec[0]
        pre_vec[7 - i] = (n >> i) & 0x01;
        i = i + 1;
    }

    pre_vec
}

fn pre_vec_gen2(n: u32) -> [u32; 8] {
    let mut pre_vec: [u32; 8] = [0; 8];
    let mut i = 0;
    while i < 8 {
        pre_vec[7 - i] = ((n >> i) & 0x01) << 16;//右移16bit
        i = i + 1;
    }

    pre_vec
}

lazy_static! {
    static ref TABLE_1: Vec<Point> = {
        let mut table: Vec<Point> = Vec::new();
        let ctx = EccCtx::new();
        for i in 0..256 {
            let p1 = ctx.mul_raw(&pre_vec_gen(i as u32), &ctx.generator());
            table.push(p1);
        }
        table
    };

    static ref TABLE_2: Vec<Point> = {
        let mut table: Vec<Point> = Vec::new();
        let ctx = EccCtx::new();
        for i in 0..256 {
            let p1 = ctx.mul_raw(&pre_vec_gen2(i as u32), &ctx.generator());
            table.push(p1);
        }
        table
    };
}

impl EccCtx {
    //初始化结构体
    pub fn new() -> EccCtx {
        let fctx = FieldCtx::new();
        EccCtx {
            fctx: FieldCtx::new(),
            a: FieldElem::new([
                0xfffffffe, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0x00000000, 0xffffffff, 0xfffffffc,
            ]),
            b: FieldElem::new([
                0x28E9FA9E, 0x9D9F5E34, 0x4D5A9E4B, 0xCF6509A7, 0xF39789F5, 0x15AB8F92, 0xDDBCBD41, 0x4D940E93,
            ]),
            n: BigUint::from_str_radix(
                "FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFF7203DF6B21C6052B53BBF40939D54123",
                16,
            ).unwrap(),
            inv2: fctx.inv(&FieldElem::from_num(2)),
        }
    }

    //a转换为u8字符串
    pub fn get_a(&self) -> Vec<u8> {
        self.a.to_bytes()
    }
    //b转换为u8字符串
    pub fn get_b(&self) -> Vec<u8> {
        self.b.to_bytes()
    }
    //n转换为大整数
    pub fn get_n(&self) -> BigUint {
        self.n.clone()
    }

    /***********逆元mod n ***********/
    // Extended Eulidean Algorithm(EEA) to calculate x^(-1) mod n；guide书底部62页，顶部41页，算法2.21
    // 扩展欧几里得方法计算逆元，参考文献 3.5节算法12：http://delta.cs.cinvestav.mx/~francisco/arith/julio.pdf
    pub fn inv_n(&self, x: &BigUint) -> BigUint {
        if x.clone() == BigUint::zero() {
            panic!("zero has no inversion.");
        }

        let mut u = x.clone();
        let mut v = self.n.clone();
        let mut a = BigUint::one();
        let mut c = BigUint::zero();

        let n = self.n.clone();
        let two = BigUint::from_u32(2).unwrap();

        while u != BigUint::zero() {
            if u.is_even() {
                u = u / two.clone();
                if a.is_even() {
                    a = a / two.clone();
                } else {
                    a = (a + n.clone()) / two.clone();
                }
            }

            if v.is_even() {
                v = v / two.clone();
                if c.is_even() {
                    c = c / two.clone();
                } else {
                    c = (c + n.clone()) / two.clone();
                }
            }

            if u >= v {
                u = u - v.clone();
                if a >= c {
                    a = a - c.clone();
                } else {
                    a = a + n.clone() - c.clone();
                }
            } else {
                v = v - u.clone();
                if c >= a {
                    c = c - a.clone();
                } else {
                    c = c + n.clone() - a.clone();
                }
            }
        }
        return c;
    }

    //判断是否为椭圆曲线仿射坐标点
    pub fn new_point(&self, x: &FieldElem, y: &FieldElem) -> Result<Point, String> {
        let ctx = &self.fctx;

        // Check if (x, y) is a valid point on the curve(affine projection)
        // y^2 = x^3 + a * x + b
        let lhs = ctx.mul(&y, &y);                                   //计算y^2
        let x_cubic = ctx.mul(&x, &ctx.mul(&x, &x));           //计算x^3
        let ax = ctx.mul(&x, &self.a);                               //计算a * x
        let rhs = ctx.add(&self.b, &ctx.add(&x_cubic, &ax));   //计算x^3 + a * x + b

        if !lhs.eq(&rhs) {  //判断是否相等
            return Err(String::from("invalid point"));
        }

        let p = Point {
            x: *x,
            y: *y,
            z: FieldElem::from_num(1),
        };
        return Ok(p); //返回坐标点p
    }

    // TODO: load point
    // pub fn load_point(&self, buf: &[u8]) -> Result<Point, bool>
    //判断是否为雅克比坐标点
    pub fn new_jacobian(
        &self,
        x: &FieldElem,
        y: &FieldElem,
        z: &FieldElem,
    ) -> Result<Point, String> {
        let ctx = &self.fctx;

        // Check if (x, y, z) is a valid point on the curve(in jacobian projection)
        // y^2 = x^3 + a * x * z^4 + b * z^6
        let lhs = ctx.square(y);  // y^2 
        let r1 = ctx.cubic(x);  // x^3 
        let r2 = ctx.mul(x, &self.a);  // a*x 
        let r2 = ctx.mul(&r2, z);  // a * x * z
        let r2 = ctx.mul(&r2, &ctx.cubic(z));  // a * x * z * z^3
        let r3 = ctx.cubic(z);  // z^3
        let r3 = ctx.square(&r3); // z^6
        let r3 = ctx.mul(&r3, &self.b); // b * z^6
        let rhs = ctx.add(&r1, &ctx.add(&r2, &r3));  // x^3 + a * x * z^4 + b * z^6

        // Require lhs =rhs
        if !lhs.eq(&rhs) {  //判断等号是否成立， y^2 = x^3 + a * x * z^4 + b * z^6
            return Err(String::from("invalid jacobian point"));
        }

        let p = Point {
            x: *x,
            y: *y,
            z: *z,
        };
        return Ok(p);
    }

    //椭圆曲线生成元
    pub fn generator(&self) -> Point {
        let x = FieldElem::new([
            0x32C4AE2C, 0x1F198119, 0x5F990446, 0x6A39C994, 0x8FE30BBF, 0xF2660BE1, 0x715A4589, 0x334C74C7,
        ]);
        let y = FieldElem::new([
            0xBC3736A2, 0xF4F6779C, 0x59BDCEE3, 0x6B692153, 0xD0A9877C, 0xC62A4740, 0x02DF32E5, 0x2139F0A0,
        ]);

        match self.new_point(&x, &y) {
            Ok(p) => p,
            Err(m) => panic!("{}", m),
        }
    }

    //零元：椭圆曲线点初始化
    pub fn zero(&self) -> Point {
        let x = FieldElem::from_num(1);
        let y = FieldElem::from_num(1);
        let z = FieldElem::zero();

        self.new_jacobian(&x, &y, &z).unwrap()
    }

    //雅克比坐标点(x,y,z)转换为仿射坐标点(x,y)
    pub fn to_affine(&self, p: &Point) -> (FieldElem, FieldElem) {
        let ctx = &self.fctx;
        if p.is_zero() {
            panic!("cannot convert the infinite point to affine");
        }

        let zinv = ctx.inv(&p.z);
        let x = ctx.mul(&p.x, &ctx.mul(&zinv, &zinv));
        let y = ctx.mul(&p.y, &ctx.mul(&zinv, &ctx.mul(&zinv, &zinv)));

        (x, y)  //返回仿射坐标点(x, y)
    }

    //负元，找对称点
    pub fn neg(&self, p: &Point) -> Point {
        let neg_y = self.fctx.neg(&p.y); //计算p-y

        match self.new_jacobian(&p.x, &neg_y, &p.z) {  //判断是否为雅克比坐标点
            Ok(neg_p) => neg_p,
            Err(e) => panic!("{}", e),
        }
    }

    //不同点相加：SM2总则：A.1.2.3.2 Jacobian加重射影坐标系底部19页 pdf13页
    pub fn add(&self, p1: &Point, p2: &Point) -> Point {
        if p1.is_zero() {
            return p2.clone();
        } else if p2.is_zero() {
            return p1.clone();
        }

        let ctx = &self.fctx;

        //if self.eq(&p1, &p2) {
        //    return self.double(p1);
        //}
        //两点交线，计算第三个点的对称点
        let lam1 = ctx.mul(&p1.x, &ctx.square(&p2.z));  //λ1=x1 * z_2^2
        let lam2 = ctx.mul(&p2.x, &ctx.square(&p1.z));  //λ2=x2 * z_1^2
        let lam3 = ctx.sub(&lam1, &lam2);  //λ3=λ1 - λ2
        let lam4 = ctx.mul(&p1.y, &ctx.cubic(&p2.z));  //λ4=y1 * z_2^3
        let lam5 = ctx.mul(&p2.y, &ctx.cubic(&p1.z));  //λ5=y2 * z_1^3
        let lam6 = ctx.sub(&lam4, &lam5);  //λ6=λ4 - λ5
        let lam7 = ctx.add(&lam1, &lam2);  //λ7=λ1 + λ1
        let lam8 = ctx.add(&lam4, &lam5);  //λ8=λ4 + λ5
        let x3 = ctx.sub(&ctx.square(&lam6), &ctx.mul(&lam7, &ctx.square(&lam3)));  //x3=λ6^2 - λ7 * λ3^2
        let lam9 = ctx.sub(&ctx.mul(&lam7, &ctx.square(&lam3)), &ctx.mul(&FieldElem::from_num(2), &x3),);  //λ9=λ7 * λ3^2 - 2*x3
        let y3 = ctx.mul(&self.inv2, &ctx.sub(&ctx.mul(&lam9, &lam6), &ctx.mul(&lam8, &ctx.cubic(&lam3))),);   //y3=(λ9 * λ6 - λ8 * λ3^3)/2
        let z3 = ctx.mul(&p1.z, &ctx.mul(&p2.z, &lam3));  //z3=z1 * z2 * λ3

        Point {
            x: x3,
            y: y3,
            z: z3,
        }
    }

    //倍点运算2p，相同点相加：SM2总则：A.1.2.3.2 Jacobian加重射影坐标系底部19页 pdf13页
    pub fn double(&self, p: &Point) -> Point {
        let ctx = &self.fctx;
        // λ1 = 3 * x1^2 + a * z1^4
        let lam1 = ctx.add(&ctx.mul(&FieldElem::from_num(3), &ctx.square(&p.x)),&ctx.mul(&self.a, &ctx.square(&ctx.square(&p.z))),);        
        let lam2 = &ctx.mul(&FieldElem::from_num(4), &ctx.mul(&p.x, &ctx.square(&p.y)));// λ2 = 4 * x1 * y1^2        
        let lam3 = &ctx.mul(&FieldElem::from_num(8), &ctx.square(&ctx.square(&p.y)));// λ3 = 8 * y1^4        
        let x3 = ctx.sub(&ctx.square(&lam1), &ctx.mul(&FieldElem::from_num(2), &lam2));// x3 = λ1^2 - 2 * λ2        
        let y3 = ctx.sub(&ctx.mul(&lam1, &ctx.sub(&lam2, &x3)), &lam3);// y3 = λ1 * (λ2 - x3) - λ3       
        let z3 = ctx.mul(&FieldElem::from_num(2), &ctx.mul(&p.y, &p.z)); // z3 = 2 * y1 * z1

        Point {
            x: x3,
            y: y3,
            z: z3,
        }
    }

    //kP，大数k乘以点P
    pub fn mul(&self, m: &BigUint, p: &Point) -> Point {
        let m = m % self.get_n();
        let k = FieldElem::from_biguint(&m);
        self.mul_raw(&k.value, p)
    }

    //kP, u32的k乘以点P，得到新的点P； SM2总则：A.3.2 Jacobian加重射影坐标系底部29页 pdf22页
    pub fn mul_raw(&self, m: &[u32], p: &Point) -> Point {
        let mut q = self.zero();

        let mut i = 0;
        while i < 256 {
            let index = i as usize / 32;
            let bit = 31 - i as usize % 32;

            //算法1：二进制展开法
            // let sum = self.add(&q0, &q1);
            q = self.double(&q);//相同点倍点运算2Q

            if (m[index] >> bit) & 0x01 != 0 {
                q = self.add(&q, &p);//不同点相加 P+Q
                // q = self.double(&q0);
            }

            i = i + 1;
        }
        q
    }

    //取出第i位
    #[inline(always)]
    fn ith_bit(n: u32, i: i32) -> u32 {
        (n >> i) & 0x01
    }


    //合成k，8个u32
    #[inline(always)]
    fn compose_k(v: &[u32], i: i32) -> u32 {
        EccCtx::ith_bit(v[7], i)
            + (EccCtx::ith_bit(v[6], i) << 1)
            + (EccCtx::ith_bit(v[5], i) << 2)
            + (EccCtx::ith_bit(v[4], i) << 3)
            + (EccCtx::ith_bit(v[3], i) << 4)
            + (EccCtx::ith_bit(v[2], i) << 5)
            + (EccCtx::ith_bit(v[1], i) << 6)
            + (EccCtx::ith_bit(v[0], i) << 7)
    }

    //基点的倍点运算k*G
    pub fn g_mul(&self, m: &BigUint) -> Point {
        let m = m % self.get_n();  //模大整数n
        let k = FieldElem::from_biguint(&m); //取出大整数m
        let mut q = self.zero();  //初始化为0

        let mut i = 15;
        while i >= 0 {
            q = self.double(&q);
            let k1 = EccCtx::compose_k(&k.value, i);
            let k2 = EccCtx::compose_k(&k.value, i + 16);
            let p1 = &TABLE_1[k1 as usize];
            let p2 = &TABLE_2[k2 as usize];
            q = self.add(&self.add(&q, p1), p2);

            i = i - 1;
        }

        q
    }

    //判断两个点是否相等
    pub fn eq(&self, p1: &Point, p2: &Point) -> bool {
        let z1 = &p1.z;
        let z2 = &p2.z;
        if z1.eq(&FieldElem::zero()) {
            if z2.eq(&FieldElem::zero()) {
                return true;
            } else {
                return false;
            }
        } else if z2.eq(&FieldElem::zero()) {
            return false;
        }

        let (p1x, p1y) = self.to_affine(p1);
        let (p2x, p2y) = self.to_affine(p2);

        if p1x.eq(&p2x) && p1y.eq(&p2y) {
            return true;
        } else {
            return false;
        }
    }

    pub fn random_uint(&self) -> BigUint {
        let mut rng = OsRng::new().unwrap();
        let mut buf: [u8; 32] = [0; 32];

        let mut ret;

        loop {
            rng.fill_bytes(&mut buf[..]);
            ret = BigUint::from_bytes_be(&buf[..]);
            if ret < self.n.clone() - BigUint::one() && ret != BigUint::zero() {
                break;
            }
        }
        ret
    }

    //点转换为字节
    pub fn point_to_bytes(&self, p: &Point, compress: bool) -> Vec<u8> {
        let (x, y) = self.to_affine(p);
        let mut ret: Vec<u8> = Vec::new();

        if compress {
            if y.get_value(7) & 0x01 == 0 {
                ret.push(0x02);
            } else {
                ret.push(0x03);
            }
            let mut x_vec = x.to_bytes();
            ret.append(&mut x_vec);
        } else {
            ret.push(0x04);
            let mut x_vec = x.to_bytes();
            let mut y_vec = y.to_bytes();
            ret.append(&mut x_vec);
            ret.append(&mut y_vec);
        }

        ret
    }

    //字节转换为点
    pub fn bytes_to_point(&self, b: &[u8]) -> Result<Point, bool> {
        let ctx = &self.fctx;

        if b.len() == 33 {
            let y_q;
            if b[0] == 0x02 {
                y_q = 0;
            } else if b[0] == 0x03 {
                y_q = 1
            } else {
                return Err(true);
            }

            let x = FieldElem::from_bytes(&b[1..]);

            let x_cubic = ctx.mul(&x, &ctx.mul(&x, &x));
            let ax = ctx.mul(&x, &self.a);
            let y_2 = ctx.add(&self.b, &ctx.add(&x_cubic, &ax));

            let mut y = self.fctx.sqrt(&y_2)?;
            if y.get_value(7) & 0x01 != y_q {
                y = self.fctx.neg(&y);
            }

            match self.new_point(&x, &y) {
                Ok(p) => {
                    return Ok(p);
                }
                Err(_) => {
                    return Err(true);
                }
            }
        } else if b.len() == 65 {
            if b[0] != 0x04 {
                return Err(true);
            }
            let x = FieldElem::from_bytes(&b[1..33]);
            let y = FieldElem::from_bytes(&b[33..65]);
            match self.new_point(&x, &y) {
                Ok(p) => {
                    return Ok(p);
                }
                Err(_) => {
                    return Err(true);
                }
            }
        } else {
            return Err(true);
        }
    }
}

impl Point {
    //判断是否为零点
    pub fn is_zero(&self) -> bool {
        if self.z.eq(&FieldElem::zero()) {
            return true;
        } else {
            return false;
        }
    }
}

use std::fmt;
impl fmt::Display for Point {
    //形式化显示仿射坐标点
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let curve = EccCtx::new();
        if self.is_zero() {
            write!(f, "(O)")
        } else {
            let (x, y) = curve.to_affine(self);
            write!(f, "(x = {}, y = {})", x.to_str(10), y.to_str(10))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_double_neg() {
        let curve = EccCtx::new();
        let g = curve.generator();

        let neg_g = curve.neg(&g);
        let double_g = curve.double(&g);
        let new_g = curve.add(&double_g, &neg_g);
        let zero = curve.add(&g, &neg_g);

        assert!(curve.eq(&g, &new_g));
        assert!(zero.is_zero());
    }

    #[test]
    fn test_multiplication() {
        let curve = EccCtx::new();
        let g = curve.generator();

        let double_g = curve.double(&g);
        let twice_g = curve.mul(&BigUint::from_u32(2).unwrap(), &g);

        assert!(curve.eq(&double_g, &twice_g));

        let n = curve.n.clone() - BigUint::one();
        let new_g = curve.mul(&n, &g);
        let new_g = curve.add(&new_g, &double_g);
        assert!(curve.eq(&g, &new_g));
    }

    #[test]
    fn test_g_multiplication() {
        let curve = EccCtx::new();
        let g = curve.generator();

        let twice_g = curve.g_mul(&BigUint::from_u64(4294967296).unwrap());
        let double_g = curve.mul(&BigUint::from_u64(4294967296).unwrap(), &g);

        assert!(curve.eq(&double_g, &twice_g));

        let n = curve.n.clone() - BigUint::one();
        let new_g = curve.g_mul(&n);
        let nn_g = curve.mul(&n, &g);
        assert!(curve.eq(&nn_g, &new_g));
    }

    #[test]
    fn test_inv_n() {
        let curve = EccCtx::new();

        for _ in 0..20 {
            let r = curve.random_uint();
            let r_inv = curve.inv_n(&r);

            let product = r * r_inv;
            let product = product % curve.get_n();

            assert_eq!(product, BigUint::one());
        }
    }

    #[test]
    fn test_point_bytes_conversion() {
        let curve = EccCtx::new();

        let g = curve.generator();
        let g_bytes_uncomp = curve.point_to_bytes(&g, false);
        let new_g = curve.bytes_to_point(&g_bytes_uncomp[..]).unwrap();
        assert!(curve.eq(&g, &new_g));
        let g_bytes_comp = curve.point_to_bytes(&g, true);
        let new_g = curve.bytes_to_point(&g_bytes_comp[..]).unwrap();
        assert!(curve.eq(&g, &new_g));

        let g = curve.double(&g);
        let g_bytes_uncomp = curve.point_to_bytes(&g, false);
        let new_g = curve.bytes_to_point(&g_bytes_uncomp[..]).unwrap();
        assert!(curve.eq(&g, &new_g));
        let g_bytes_comp = curve.point_to_bytes(&g, true);
        let new_g = curve.bytes_to_point(&g_bytes_comp[..]).unwrap();
        assert!(curve.eq(&g, &new_g));

        let g = curve.double(&g);
        let g_bytes_uncomp = curve.point_to_bytes(&g, false);
        let new_g = curve.bytes_to_point(&g_bytes_uncomp[..]).unwrap();
        assert!(curve.eq(&g, &new_g));
        let g_bytes_comp = curve.point_to_bytes(&g, true);
        let new_g = curve.bytes_to_point(&g_bytes_comp[..]).unwrap();
        assert!(curve.eq(&g, &new_g));
    }
}
