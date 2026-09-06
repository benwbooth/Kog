#include <lib/std.mi>
#include <lib/fileio.mi>
#define DEF_MAX 50

Function ProcessMenuResult(int a);
Function refreshView();
Function showGroup(int groupNo);
Function startTimer();
Function setAnimNo(int i);
Function setCustomVis(int a);

Global Group frameGroup, beatGroup, promoGroup, b01, b02;

Global AnimatedLayer beatAnimLeft, beatAnimRight;
Global Timer myTimer;
Global int lastBeatLeft,lastBeatRight, myFrames, aniW, beatLeft, beatRight, frameLeft, run_max1, frameRight, cusno; 
Global Boolean showBeat, showPromo, animTypeB, oneSide, customvis;
Global Layer promoPic, mouseTrap, b01layer, b02layer, c01, c02;
Global Popupmenu selMenu;
Global XmlDoc myDoc;
Global List cusbeat_names;
Global float run_max;

System.onScriptLoaded (){
	frameGroup = getScriptGroup ();

	// mpdeimos> taking advance of grouping and get instead of find will save lots of CPU cycles and skins are more resistant against mods!

	Group cg = frameGroup.getObject("cpro.screen");
	
	beatGroup = cg.getObject("beatvis");
	promoGroup = cg.getObject("beatpromo");
	promoPic = promoGroup.getObject("beat.promo");
	mouseTrap = cg.getObject("beat.mousetrap");

	beatAnimLeft = beatGroup.getObject("beatvis.left");
	beatAnimRight = beatGroup.getObject("beatvis.right");
	b01 = beatGroup.getObject("beatvisB.left");
	b02 = beatGroup.getObject("beatvisB.right");
	b01layer = b01.getObject("beatvisB.left.layer");
	b02layer = b02.getObject("beatvisB.right.layer");
	c01 = beatGroup.getObject("beatvisC.left");
	c02 = beatGroup.getObject("beatvisC.right");
	
	myTimer = new Timer;
	myTimer.setDelay(10);
 
	///////////////////
	Map myMap = new Map;
	myMap.loadMap("beat.0.left");
	aniW=myMap.getWidth();

	beatGroup.setXmlParam("w", integerToString(aniW*2));
	beatAnimLeft.setXmlParam("w", integerToString(aniW));
	beatAnimRight.setXmlParam("x", integerToString(aniW));
	beatAnimRight.setXmlParam("w", integerToString(aniW));
	b01layer.setXmlParam("x", integerToString(-aniW));
	c02.setXmlParam("x", integerToString(aniW));
	
	if(b02layer.isInvalid()) oneSide=true;
	
	
	//Custom Vis code
	myDoc = new XmlDoc;
	String fullpath = getParam()+"classicpro.xml";
	myDoc.load (fullpath);
	
	// If we include more stuff in the classicpro.xml at a later stage the parser must set a boolean = true to know that the xml was for this
	if(myDoc.exists() && myMap.getHeight()>90){
		cusbeat_names = new List;
		myDoc.parser_addCallback("BeatVis/*");
		myDoc.parser_addCallback("ClassicPro/Visualization/BeatVis*");
		myDoc.parser_start();
		myDoc.parser_destroy();
		if(cusbeat_names.getNumItems()>0) customvis=true;
		
		setCustomVis(getPrivateInt(getSkinName(), "customvis", 0));
		mouseTrap.setXmlParam("tooltip", "Right-Click for more animations");
	}
	else customvis=false;
	delete myDoc;

	
	if(myMap.getHeight()<=90){
		animTypeB=true;
		beatAnimLeft.hide();
		beatAnimRight.hide();
		b01.show();
		
		if(myMap.getHeight()==90) c01.show();
		
		if(oneSide){
			b01.setXmlParam("x", integerToString(aniW/2));
		}
		else{
			b02.show();
			b02.setXmlParam("x", integerToString(aniW));
			if(myMap.getHeight()==90) c02.show();
		}
	}
	else{
		animTypeB=false;
		myFrames = beatAnimLeft.getLength();
	}
	
	delete myMap;


}

myDoc.parser_onCallback (String xmlpath, String xmltag, list paramname, list paramvalue){
	if(strlower(xmltag) == "customvis"){
		for(int i=0; i<paramname.getNumItems(); i++){
			if(strlower(paramname.enumItem(i))=="name"){
				cusbeat_names.addItem(paramvalue.enumItem(i));
			}
		}
	}
}

System.onscriptunloading(){
	delete myTimer;
}

setAnimNo(int i){
	Map myMap = new Map;
	myMap.loadMap("beat.left");
	
	beatAnimLeft.setXmlParam("image", "beat.left#.0");
	beatAnimRight.setXmlParam("image", "beat.left#.0");
	myFrames = beatAnimLeft.getLength();

}

myTimer.onTimer(){

	beatLeft = System.getLeftVuMeter();
	beatRight = System.getRightVuMeter();
	
	if(oneSide)
	{
		beatLeft=(beatLeft+beatRight)/2;
	}
	
	
	
	if (beatLeft > run_max) run_max = beatLeft;
	if (beatRight > run_max) run_max = beatRight;
	run_max = run_max*0.9999;
	if (run_max < 50) run_max = 50;
	
	
	//System.SetclipboardText(floatToString(run_max,5));

	if(animTypeB){
		beatLeft=aniW/run_max*beatLeft;
		b01.setXmlParam("w", integerToString(beatLeft));
		b01.setXmlParam("x", integerToString(aniW-beatLeft));
		if(!oneSide) b02.setXmlParam("w", integerToString(aniW/run_max*beatRight));
	}
	else
	{	
		frameLeft=((beatLeft/(run_max*1))*myFrames);
		frameRight=((beatRight/(run_max*1))*myFrames);


		if (frameLeft<lastBeatLeft){
			frameLeft=lastBeatLeft-1;
		}
		if (frameRight<lastBeatRight) {
			frameRight=lastBeatRight-1;
		}
		
		if (frameLeft>lastBeatLeft+2){
			frameLeft=lastBeatLeft+3;
		}
		if (frameRight>lastBeatRight+2) {
			frameRight=lastBeatRight+3;
		}

		// Martin> Frames go from 0 to myFrames-1 !!!
		if (frameLeft>=myFrames) frameLeft=myFrames-1;
		if (frameRight>=myFrames) frameRight=myFrames-1;
		
		if(frameLeft<0) frameLeft=0;
		if(frameRight<0) frameRight=0;
		

		lastBeatLeft=frameLeft;
		lastBeatRight=frameRight;
		
		//debuger+=integerToString(frameLeft)+", ";
		//System.SetclipboardText(debuger);
		beatAnimLeft.gotoframe(frameLeft);
		beatAnimRight.gotoframe(frameRight);
	}
}

System.onTitleChange(String newTxt){
	run_max=DEF_MAX;
}

System.onStop(){
	refreshView();
	myTimer.stop();
	beatAnimLeft.gotoframe(0);
	beatAnimRight.gotoframe(0);
}

System.onPlay(){
	refreshView();
	if(beatGroup.isVisible()) startTimer();
}
System.onPause(){
	refreshView();
	if(beatGroup.isVisible()) myTimer.stop();
}

System.onResume(){
	refreshView();
	if(beatGroup.isVisible()) startTimer();
}

beatGroup.onSetVisible(Boolean onoff){
	if(onoff && System.getStatus() == STATUS_PLAYING) startTimer();
	else myTimer.stop();
}

frameGroup.onResize(int x, int y, int w, int h){

	if(oneSide){
		beatGroup.setXmlParam("x", integerToString(143+(w-317)/2-aniW/2));
		h=aniW;
	}
	else{
		beatGroup.setXmlParam("x", integerToString(143+(w-317)/2-aniW));
		h=aniW*2;
	}

	
	if(w>317+h)	showBeat=true;
	else showBeat=false;

	if(w>618){
		promoPic.setXmlParam("image", "cPro.promo.3");
		promoPic.resize(0,0,300,45);
	}
	else if(w>517){
		promoPic.setXmlParam("image", "cPro.promo.2");
		promoPic.resize(50,0,200,45);
	}
	else{
		promoPic.setXmlParam("image", "cPro.promo.1");
		promoPic.resize(150,0,99,45);
	}
	promoGroup.setXmlParam("x", integerToString(143+(w-317)/2-promoPic.getWidth()/2));
	
	if(w>416) showPromo=true;
	else showPromo=false;

	refreshView();
}

refreshView(){
	if(System.getStatus() == STATUS_STOPPED) showGroup(1);
	else if(getPrivateInt(getSkinName(), "beatvis", 1)==0)	showGroup(1);
	else showGroup(0);
}

showGroup(int groupNo){
	beatGroup.hide();
	promoGroup.hide();

	if(groupNo == 0 && showBeat) beatGroup.show();
	else if(groupNo == 1 && showPromo) promoGroup.show();
	else if(showPromo) promoGroup.show();
}

mouseTrap.onLeftButtonDblClk(int x, int y){
	if(customvis){
		if(getPrivateInt(getSkinName(), "beatvis", 1)){
			int u = getPrivateInt(getSkinName(), "customvis", 0)+1;

			if(u==cusbeat_names.getNumItems()){
				u=0;
				setPrivateInt(getSkinName(), "beatvis", 0);
			}
			setCustomVis(u);
		}
		else setPrivateInt(getSkinName(), "beatvis", 1);
	}
	else setPrivateInt(getSkinName(), "beatvis", !getPrivateInt(getSkinName(), "beatvis", 1));
	
	refreshView();
}

mouseTrap.onRightButtonup(int x, int y){
	selMenu = new PopupMenu;
	selMenu.addCommand("Show Beat vis", 1, getPrivateInt(getSkinName(), "beatvis", 1), 0);

	if(customvis){
		selMenu.addSeparator();
		for(int i=0;i<cusbeat_names.getNumItems(); i++){
			selMenu.addCommand(cusbeat_names.enumItem(i), 100+i, 0, 0);
	
		}
		selMenu.checkCommand(100+getPrivateInt(getSkinName(), "customvis", 0), 1);
	}

	ProcessMenuResult(selMenu.popAtMouse());
	Complete;
	delete selMenu;
	
}

ProcessMenuResult(int a){
	if(a<0) return;

	if(a>=1 && a<100){
		setPrivateInt(getSkinName(), "beatvis", !getPrivateInt(getSkinName(), "beatvis", 1));
		refreshView();
	}
	else if(a>=100 && customvis){
		setCustomVis(a-100);
	}
}

setCustomVis(int a){
	//debugString("setCustomVis=" + integerToString(a),9);
	if(a>=cusbeat_names.getNumItems()) a=0;
	beatAnimLeft.setXmlParam("image", "beat."+integerToString(a)+".left");
	if(!oneSide) beatAnimRight.setXmlParam("image", "beat."+integerToString(a)+".right");
	myFrames = beatAnimLeft.getLength();
	
	setPrivateInt(getSkinName(), "customvis", a);
}



startTimer(){
	run_max=DEF_MAX;
	myTimer.start();
}